//! Card faces as the original composes them (0x1DBA80): GS sprites drawn
//! into a 256×368 texture (large: the record's flag 0x81 bit 4, the deck
//! select's stack) or a 120×180 texture (small: hand cards, 0x1AFAC0). Source facts (PAL SLES_515.79, private
//! research `duel/claude-card-face/`; [F] code/data, [emu] interpreter):
//!
//! - Mode constants (0x1DBB04..0x1DBB68): large s0 = 1, star entry 0x1F,
//!   digit step 22, star size 16, card 256×368; small s0 = 0, star entry
//!   0x20, digit step 20, star size 8, card 120×180 [F].
//! - Frame: entry `10·s0 + category` of the descriptor table 0x28F1B0
//!   (TEXSYS record, u0, v0, u1, v1), `category` = card table byte 0 >> 5
//!   (0x1DE288, 0x1DC03C); monsters whose effect query 0x1E1C60 returns an
//!   entry use +6 (0x1DC1F4..0x1DC20C): off the board that is the card
//!   table's effect index (+4) other than 0xFFFF (0x1E1D88). TEXSYS #0 stays
//!   resident; other records are uploaded to the same place (0x1DAEC0) [F].
//! - Stars (category 0 only, count = level 0x1E1BD0): descriptor 0x28F300 +
//!   10·star entry, star `j` at x = stars.x + (size + s0)·(11 − j)
//!   (0x1DC4E4..0x1DC60C) [F].
//! - Picture: PICTURE.MRG record = card id, 128² drawn 192² (small 96²) at
//!   the layout's picture point with nearest sampling (0x1B8940(card, x, y,
//!   size, size)) [F; emu: VRAM of savestate 437 equals record 612].
//! - Icon and name (large only): descriptor 0x28F2A0 +
//!   10·0x1DF320(category, attribute) at the icon point, 24² (categories
//!   1,2 → 6, 3,4 → 7, 5 → 8, monsters → attribute) [F].
//! - Name: message 323 + card rendered into the text page row 9 (y 198,
//!   0x245BB0) and drawn at the layout's name point, height 22, width
//!   min(text width, 192) with bilinear sampling, colour (0, 0, 0, 96)
//!   (0x246B90) [emu].
//! - ATK/DEF (category 0 only): labels (messages 162/163 in text rows 1/2,
//!   36 wide, drawn 22 wide) and up to four 22² digit glyphs (message 22 in
//!   row 0, cell = 22·digit) right to left from x = digits.x + 4·step, stopping
//!   when the value is exhausted (at least one digit); values = base +
//!   terrain bonus clamped to 0..9999 (0x1DC14C..0x1DC1E8) [F/emu].
//! - Blend (A − B)·C + D = (Cs − Cd)·As/128 + Cd with alpha test "greater
//!   than 0" except the picture; texture function MODULATE; FBA sets the
//!   written alpha's top bit (the face is opaque wherever drawn) [emu: GS
//!   packet of 0x1DBA80].
//!
//! - Card 0x29F draws only the card back; card 675's values carry a record
//!   bonus written by 0x1DDFE0 that is not modelled (the caller passes
//!   table values plus its own bonus) [F/emu].
//!
//! Layout blocks (0x2BE0D0 + 32·s0: name, icon, stars, picture, ATK label,
//! ATK digits, DEF label, DEF digits) and descriptors are read from the
//! executable the user supplies. Oracles (all 854 cards): `faces-all.json`
//! (large, `oracle_faces.py`, savestate 437-deck1) and `small-all.json`
//! (small, `oracle_small.py`, savestate 683-p2-x1).
use roses_assets::RgbaImage;
use serde::Serialize;

pub type Result<T> = std::result::Result<T, String>;

pub const FACE_WIDTH: u32 = 256;
pub const FACE_HEIGHT: u32 = 368;
pub const EE_TO_FILE: u32 = 0x000F_9680;
pub const FRAME_TABLE_EE: u32 = 0x0028_F1B0;
pub const ICON_TABLE_EE: u32 = 0x0028_F2A0;
pub const STAR_TABLE_EE: u32 = 0x0028_F300;
pub const LAYOUT_EE: u32 = 0x002B_E0D0;
pub const CARD_TABLE_EE: u32 = 0x002B_E400;
pub const CARD_RECORD_BYTES: u32 = 20;
pub const CARDS: u16 = 854;
/// First card-name message (0x1DF480: card + 0x143).
pub const NAME_MESSAGE_BASE: u16 = 0x143;
/// Text page rows (0x245BB0 slots × 22): digits message, ATK, DEF, name.
pub const DIGITS_MESSAGE: u16 = 0x16;
pub const ATK_MESSAGE: u16 = 0xA2;
pub const DEF_MESSAGE: u16 = 0xA3;
pub const ROW_HEIGHT: u32 = 22;
pub const NAME_ROW: u32 = 9;
pub const NAME_MAX_WIDTH: u32 = 192;
pub const LABEL_WIDTH: u32 = 22;
pub const DIGIT_CELL: u32 = 22;
pub const DIGIT_SLOTS: u32 = 4;
/// The face reads the first ten cells of the digits row (0..=9).
pub const DIGIT_GLYPHS: usize = 10;
pub const STAR_SLOTS: u32 = 12;
pub const PICTURE_SOURCE: u32 = 128;
pub const ICON_SIZE: i32 = 24;
pub const EFFECT_FRAME_OFFSET: usize = 6;
pub const TEXT_COLOUR: [u8; 4] = [0, 0, 0, 96];
pub const MAX_VALUE: i32 = 9999;
/// Card 0x29F is drawn as the back (large frame entry 7) [F 0x1DBCA4].
pub const BACK_CARD: u16 = 0x29F;
pub const BACK_FRAME: usize = 7;

/// The two sizes of the composer (record flag 0x81 bit 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum FaceSize {
    Large,
    Small,
}

impl FaceSize {
    /// s0 of 0x1DBA80 (1 large, 0 small).
    pub fn s0(self) -> u32 {
        match self {
            Self::Large => 1,
            Self::Small => 0,
        }
    }
    pub fn dimensions(self) -> (u32, u32) {
        match self {
            Self::Large => (FACE_WIDTH, FACE_HEIGHT),
            Self::Small => (120, 180),
        }
    }
    fn star_entry(self) -> u32 {
        match self {
            Self::Large => 0x1F,
            Self::Small => 0x20,
        }
    }
    fn star_size(self) -> i32 {
        match self {
            Self::Large => 16,
            Self::Small => 8,
        }
    }
    fn digit_step(self) -> u32 {
        match self {
            Self::Large => 22,
            Self::Small => 20,
        }
    }
    fn picture_size(self) -> i32 {
        match self {
            Self::Large => 192,
            Self::Small => 96,
        }
    }
}

/// A 2D texture descriptor (texture slot, u0, v0, u1, v1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Descriptor {
    pub slot: u8,
    pub uv: [u16; 4],
}

/// Tables of one size.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ModeTables {
    /// Frames indexed by category (+6 for effect monsters).
    pub frames: Vec<Descriptor>,
    pub star: Descriptor,
    /// name, icon, stars, picture, ATK label, ATK digits, DEF label, DEF digits.
    pub layout: [[i32; 2]; 8],
}

/// Tables of the executable used by the faces.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FaceTables {
    pub large: ModeTables,
    pub small: ModeTables,
    /// 0x28F2A0: attribute icons 0..=5, card-type icons 6..=8.
    pub icons: Vec<Descriptor>,
}

impl FaceTables {
    pub fn mode(&self, size: FaceSize) -> &ModeTables {
        match size {
            FaceSize::Large => &self.large,
            FaceSize::Small => &self.small,
        }
    }
}

fn halfwords(exe: &[u8], ee: u32, count: usize) -> Result<Vec<i16>> {
    let start = ee
        .checked_sub(EE_TO_FILE)
        .ok_or("EE address below the executable image")? as usize;
    let bytes = exe
        .get(start..start + 2 * count)
        .ok_or_else(|| format!("executable too short for {ee:#x}"))?;
    Ok(bytes
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]))
        .collect())
}

fn descriptor(v: &[i16]) -> Result<Descriptor> {
    if v[0] < 0
        || v[0] > 15
        || v[1..].iter().any(|&x| !(0..=512).contains(&x))
        || v[1] >= v[3]
        || v[2] >= v[4]
    {
        return Err(format!(
            "card face descriptor {v:?} is not a texture rectangle"
        ));
    }
    Ok(Descriptor {
        slot: v[0] as u8,
        uv: [v[1] as u16, v[2] as u16, v[3] as u16, v[4] as u16],
    })
}

fn mode_tables(exe: &[u8], size: FaceSize) -> Result<ModeTables> {
    let s0 = size.s0();
    let frames = halfwords(exe, FRAME_TABLE_EE + 100 * s0, 50)?
        .chunks_exact(5)
        .map(descriptor)
        .collect::<Result<Vec<_>>>()?;
    let star = descriptor(&halfwords(exe, STAR_TABLE_EE + 10 * size.star_entry(), 5)?)?;
    let raw = halfwords(exe, LAYOUT_EE + 32 * s0, 16)?;
    let layout: [[i32; 2]; 8] =
        std::array::from_fn(|k| [i32::from(raw[2 * k]), i32::from(raw[2 * k + 1])]);
    let (w, h) = size.dimensions();
    if layout
        .iter()
        .any(|p| !(0..w as i32).contains(&p[0]) || !(0..h as i32).contains(&p[1]))
    {
        return Err(format!("{size:?} card layout block lies outside the face"));
    }
    if frames
        .iter()
        .any(|f| u32::from(f.uv[2] - f.uv[0]) != w || u32::from(f.uv[3] - f.uv[1]) != h)
    {
        return Err(format!("{size:?} frame descriptors are not {w}×{h}"));
    }
    Ok(ModeTables {
        frames,
        star,
        layout,
    })
}

impl FaceTables {
    pub fn from_executable(exe: &[u8]) -> Result<Self> {
        let icons = halfwords(exe, ICON_TABLE_EE, 45)?
            .chunks_exact(5)
            .map(descriptor)
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            large: mode_tables(exe, FaceSize::Large)?,
            small: mode_tables(exe, FaceSize::Small)?,
            icons,
        })
    }
}

/// Card table bytes the face reads (0x2BE400 + 20·card).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct CardRecordBytes {
    /// byte 0 >> 5.
    pub category: u8,
    /// byte 2 & 15.
    pub attribute: u8,
    /// Halfword +4: effect index, 0xFFFF for none.
    pub effect_index: u16,
}

impl CardRecordBytes {
    /// 0x1E1C60 off the board: an effect entry exists.
    pub fn has_effect(&self) -> bool {
        self.effect_index != 0xFFFF
    }
}

pub fn card_record(exe: &[u8], card: u16) -> Result<CardRecordBytes> {
    if card >= CARDS {
        return Err(format!("card {card} is outside the card table"));
    }
    let start = (CARD_TABLE_EE - EE_TO_FILE + u32::from(card) * CARD_RECORD_BYTES) as usize;
    let bytes = exe
        .get(start..start + CARD_RECORD_BYTES as usize)
        .ok_or("executable too short for the card table")?;
    Ok(CardRecordBytes {
        category: bytes[0] >> 5,
        attribute: bytes[2] & 15,
        effect_index: u16::from_le_bytes([bytes[4], bytes[5]]),
    })
}

/// What one card face needs beyond the tables.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FaceCard {
    pub card: u16,
    pub record: CardRecordBytes,
    /// 0x1E1BD0.
    pub level: u8,
    /// 0x1E1570 / 0x1E1650 plus terrain bonus (the caller's), unclamped.
    pub attack: i32,
    pub defense: i32,
    /// 0x1E1C60.
    pub effect: bool,
    /// Width of the rendered name row (12 per narrow glyph, 22 per wide).
    pub name_width: u32,
}

/// Texture a face sprite samples.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum FaceTexture {
    /// A TEXSYS record (frames: #0..#3, icons/stars: #6).
    Texsys(u8),
    Picture,
    /// The text page: digits row 0, ATK row 1, DEF row 2, name row 9.
    TextPage,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct FaceSprite {
    pub texture: FaceTexture,
    /// Texel rectangle (u0, v0, u1, v1).
    pub uv: [u32; 4],
    /// Face pixel rectangle (x0, y0, x1, y1).
    pub rect: [i32; 4],
    pub modulate: [u8; 4],
    pub bilinear: bool,
    /// Alpha test "greater than 0" (KEEP on fail).
    pub alpha_test: bool,
}

fn sprite(texture: FaceTexture, uv: [u32; 4], rect: [i32; 4]) -> FaceSprite {
    FaceSprite {
        texture,
        uv,
        rect,
        modulate: [128; 4],
        bilinear: false,
        alpha_test: true,
    }
}

fn descriptor_sprite(d: Descriptor, at: [i32; 2], size: [i32; 2]) -> FaceSprite {
    sprite(
        FaceTexture::Texsys(d.slot),
        d.uv.map(u32::from),
        [at[0], at[1], at[0] + size[0], at[1] + size[1]],
    )
}

/// The ordered sprite list of 0x1DBA80.
pub fn face_sprites(
    card: &FaceCard,
    tables: &FaceTables,
    size: FaceSize,
) -> Result<Vec<FaceSprite>> {
    let mode = tables.mode(size);
    let (w, h) = size.dimensions();
    let full = [w as i32, h as i32];
    if card.card == BACK_CARD {
        // 0x1DBCA4 → 0x1DCBA4: the card back entry only.
        let back = *mode
            .frames
            .get(BACK_FRAME)
            .ok_or("the frame table has no card back")?;
        return Ok(vec![descriptor_sprite(back, [0, 0], full)]);
    }
    let category = usize::from(card.record.category);
    let monster = category == 0;
    let mut frame_index = category;
    if monster && card.effect {
        frame_index += EFFECT_FRAME_OFFSET;
    }
    let frame = *mode
        .frames
        .get(frame_index)
        .ok_or_else(|| format!("card {} has no frame {frame_index}", card.card))?;
    let [name_at, icon_at, stars_at, picture_at, atk_label, atk_digits, def_label, def_digits] =
        mode.layout;
    let large = size == FaceSize::Large;
    let star_size = size.star_size();
    let star_step = star_size + size.s0() as i32;
    let picture_size = size.picture_size();
    let mut out = vec![descriptor_sprite(frame, [0, 0], full)];
    if monster {
        if u32::from(card.level) > STAR_SLOTS {
            return Err(format!(
                "card {} level {} exceeds the stars",
                card.card, card.level
            ));
        }
        for j in 0..i32::from(card.level) {
            let k = STAR_SLOTS as i32 - 1 - j;
            out.push(descriptor_sprite(
                mode.star,
                [stars_at[0] + k * star_step, stars_at[1]],
                [star_size, star_size],
            ));
        }
    }
    let mut picture = sprite(
        FaceTexture::Picture,
        [0, 0, PICTURE_SOURCE, PICTURE_SOURCE],
        [
            picture_at[0],
            picture_at[1],
            picture_at[0] + picture_size,
            picture_at[1] + picture_size,
        ],
    );
    picture.alpha_test = false;
    out.push(picture);
    // 0x1DF320.
    let icon_index = match category {
        1 | 2 => 6,
        3 | 4 => 7,
        5 => 8,
        0 => usize::from(card.record.attribute),
        _ => category + 5,
    };
    let icon = *tables
        .icons
        .get(icon_index)
        .ok_or_else(|| format!("card {} has no icon {icon_index}", card.card))?;
    if large {
        out.push(descriptor_sprite(icon, icon_at, [ICON_SIZE, ICON_SIZE]));
    }
    let text = |uv: [u32; 4], rect: [i32; 4]| FaceSprite {
        texture: FaceTexture::TextPage,
        uv,
        rect,
        modulate: TEXT_COLOUR,
        bilinear: true,
        alpha_test: true,
    };
    let name_y = NAME_ROW * ROW_HEIGHT;
    if large && card.name_width > 0 {
        let drawn = card.name_width.min(NAME_MAX_WIDTH) as i32;
        out.push(text(
            [0, name_y, card.name_width, name_y + ROW_HEIGHT],
            [
                name_at[0],
                name_at[1],
                name_at[0] + drawn,
                name_at[1] + ROW_HEIGHT as i32,
            ],
        ));
    }
    if monster {
        for (row, label, digits, value) in [
            (1, atk_label, atk_digits, card.attack),
            (2, def_label, def_digits, card.defense),
        ] {
            out.push(text(
                [0, row * ROW_HEIGHT, 36, (row + 1) * ROW_HEIGHT],
                [
                    label[0],
                    label[1],
                    label[0] + LABEL_WIDTH as i32,
                    label[1] + ROW_HEIGHT as i32,
                ],
            ));
            let mut value = value.clamp(0, MAX_VALUE) as u32;
            for slot in 0..DIGIT_SLOTS {
                let digit = value % 10;
                let x = digits[0] + ((DIGIT_SLOTS - slot) * size.digit_step()) as i32;
                out.push(text(
                    [digit * DIGIT_CELL, 0, (digit + 1) * DIGIT_CELL, ROW_HEIGHT],
                    [
                        x,
                        digits[1],
                        x + DIGIT_CELL as i32,
                        digits[1] + ROW_HEIGHT as i32,
                    ],
                ));
                value /= 10;
                if value == 0 {
                    break;
                }
            }
        }
    }
    Ok(out)
}

/// RGBA textures in GS units (RGB 0..=255 from 5-bit << 3, A 0..=128).
pub struct FaceTextures<'a> {
    pub texsys: &'a dyn Fn(u8) -> Option<&'a RgbaImage>,
    pub picture: Option<&'a RgbaImage>,
    pub text_page: &'a RgbaImage,
}

fn texel(image: &RgbaImage, u: i64, v: i64) -> [i32; 4] {
    let x = u.clamp(0, i64::from(image.width) - 1) as usize;
    let y = v.clamp(0, i64::from(image.height) - 1) as usize;
    let at = (y * image.width as usize + x) * 4;
    let p = &image.pixels[at..at + 4];
    [
        i32::from(p[0]),
        i32::from(p[1]),
        i32::from(p[2]),
        i32::from(p[3]),
    ]
}

/// The GS bilinear sample of `image` at texel coordinates (u, v) (4-bit
/// weights, clamped edges), as the compositor uses it.
pub fn sample_bilinear(image: &RgbaImage, u: f64, v: f64) -> [i32; 4] {
    sample(image, u, v, true)
}

fn sample(image: &RgbaImage, u: f64, v: f64, bilinear: bool) -> [i32; 4] {
    if !bilinear {
        return texel(image, u.floor() as i64, v.floor() as i64);
    }
    let (us, vs) = (u - 0.5, v - 0.5);
    let (u0, v0) = (us.floor(), vs.floor());
    let (fu, fv) = (((us - u0) * 16.0) as i32, ((vs - v0) * 16.0) as i32);
    let (u0, v0) = (u0 as i64, v0 as i64);
    let a = texel(image, u0, v0);
    let b = texel(image, u0 + 1, v0);
    let c = texel(image, u0, v0 + 1);
    let d = texel(image, u0 + 1, v0 + 1);
    std::array::from_fn(|i| {
        let top = a[i] * (16 - fu) + b[i] * fu;
        let bottom = c[i] * (16 - fu) + d[i] * fu;
        (top * (16 - fv) + bottom * fv) >> 8
    })
}

/// Draw the sprites into a face of `size`: RGB as the GS target, A = 255
/// where any sprite wrote (FBA), else 0. A missing picture leaves its area
/// as the frame window.
pub fn compose(
    sprites: &[FaceSprite],
    textures: &FaceTextures,
    size: FaceSize,
) -> Result<RgbaImage> {
    let (fw, fh) = size.dimensions();
    let (w, h) = (fw as usize, fh as usize);
    let mut rgb = vec![[0i32; 3]; w * h];
    let mut written = vec![false; w * h];
    for s in sprites {
        let image = match s.texture {
            FaceTexture::Texsys(slot) => {
                (textures.texsys)(slot).ok_or_else(|| format!("TEXSYS #{slot} is not available"))?
            }
            FaceTexture::Picture => match textures.picture {
                Some(p) => p,
                None => continue,
            },
            FaceTexture::TextPage => textures.text_page,
        };
        let [x0, y0, x1, y1] = s.rect;
        let du = f64::from(s.uv[2] - s.uv[0]) / f64::from(x1 - x0);
        let dv = f64::from(s.uv[3] - s.uv[1]) / f64::from(y1 - y0);
        for y in y0.max(0)..y1.min(h as i32) {
            let v = f64::from(s.uv[1]) + 0.5 + f64::from(y - y0) * dv;
            for x in x0.max(0)..x1.min(w as i32) {
                let u = f64::from(s.uv[0]) + 0.5 + f64::from(x - x0) * du;
                let t = sample(image, u, v, s.bilinear);
                let m = s.modulate.map(i32::from);
                let colour: [i32; 3] = std::array::from_fn(|i| ((t[i] * m[i]) >> 7).min(255));
                let alpha = ((t[3] * m[3]) >> 7).min(255);
                if s.alpha_test && alpha <= 0 {
                    continue;
                }
                let at = y as usize * w + x as usize;
                let d = rgb[at];
                rgb[at] = std::array::from_fn(|i| {
                    (((colour[i] - d[i]) * alpha) >> 7)
                        .saturating_add(d[i])
                        .clamp(0, 255)
                });
                written[at] = true;
            }
        }
    }
    let mut pixels = Vec::with_capacity(w * h * 4);
    for (c, on) in rgb.iter().zip(&written) {
        pixels.extend([
            c[0] as u8,
            c[1] as u8,
            c[2] as u8,
            if *on { 255 } else { 0 },
        ]);
    }
    Ok(RgbaImage {
        width: fw,
        height: fh,
        pixels,
    })
}

/// 0x2D2F80: alpha of each 4-bit glyph intensity in the text CLUT built by
/// 0x245A04 (GS units) [F].
pub const TEXT_ALPHA_RAMP_EE: u32 = 0x002D_2F80;
pub const TEXT_PAGE_WIDTH: u32 = 1024;
pub const TEXT_PAGE_ROWS: u32 = 10;
pub const NARROW_ADVANCE: u32 = 12;
pub const WIDE_ADVANCE: u32 = 22;

pub fn text_alpha_ramp(exe: &[u8]) -> Result<[u8; 16]> {
    let start = (TEXT_ALPHA_RAMP_EE - EE_TO_FILE) as usize;
    let bytes = exe
        .get(start..start + 16)
        .ok_or("executable too short for the text alpha ramp")?;
    let ramp: [u8; 16] = bytes.try_into().unwrap();
    if ramp[0] != 0 || ramp.windows(2).any(|w| w[1] < w[0]) || ramp[15] > 128 {
        return Err("text alpha ramp is not an increasing GS alpha ramp".into());
    }
    Ok(ramp)
}

/// Render one message line into `page` at row `row` as 0x245BB0 does: a
/// narrow code (0x2001..=0x20A3) copies the left 12 columns of its 22² cell
/// and advances 12, a wide code (0xA4..=0x13C) copies the full cell and
/// advances 22. The font image holds the 4-bit intensity as alpha · 17; the
/// page stores white with the ramp's GS alpha. Returns the row width.
pub fn render_text_row(
    page: &mut RgbaImage,
    font: &roses_assets::MessageFont,
    codes: &[u16],
    row: u32,
    ramp: &[u8; 16],
) -> Result<u32> {
    let mut x = 0u32;
    for &code in codes {
        if code == 0x2000 {
            break;
        }
        let (cell, advance) = if (0x2001..=0x20A3).contains(&code) {
            (font.glyph_rect(code), NARROW_ADVANCE)
        } else {
            (font.name_glyph_rect(code), WIDE_ADVANCE)
        };
        let cell = cell.ok_or_else(|| format!("text code {code:#06x} has no font cell"))?;
        if x + advance > page.width {
            return Err("text row exceeds the text page".into());
        }
        for y in 0..ROW_HEIGHT {
            for dx in 0..advance {
                let from = (((cell[1] + y) * font.image.width + cell[0] + dx) * 4) as usize;
                let intensity = font.image.pixels[from + 3] / 17;
                let to = (((row * ROW_HEIGHT + y) * page.width + x + dx) * 4) as usize;
                page.pixels[to..to + 4].copy_from_slice(&[
                    255,
                    255,
                    255,
                    ramp[usize::from(intensity)],
                ]);
            }
        }
        x += advance;
    }
    Ok(x)
}

/// A blank text page (white, alpha 0).
pub fn blank_text_page() -> RgbaImage {
    let mut pixels = vec![0u8; (TEXT_PAGE_WIDTH * TEXT_PAGE_ROWS * ROW_HEIGHT * 4) as usize];
    for p in pixels.chunks_exact_mut(4) {
        p[..3].copy_from_slice(&[255, 255, 255]);
    }
    RgbaImage {
        width: TEXT_PAGE_WIDTH,
        height: TEXT_PAGE_ROWS * ROW_HEIGHT,
        pixels,
    }
}

/// Convert an RGBA image whose alpha is 0 or 255 (PNG convention of the
/// imports) to GS units (0 or 128).
pub fn to_gs_alpha(mut image: RgbaImage) -> RgbaImage {
    for p in image.pixels.chunks_exact_mut(4) {
        p[3] = if p[3] >= 128 { 128 } else { 0 };
    }
    image
}

#[cfg(test)]
mod tests;

/// GS-unit RGBA of an indexed TEXSYS-style texture (5-bit << 3, alpha bit →
/// 128).
pub fn indexed_gs_rgba(texture: &roses_assets::OpeningIndexedTexture) -> Result<RgbaImage> {
    if texture.palette_format != roses_assets::OpeningPaletteFormat::Rgb5551 {
        return Err("card textures with 32-bit palettes are not supported".into());
    }
    let mut pixels = Vec::with_capacity(texture.palette_indices.len() * 4);
    for index in &texture.palette_indices {
        let v = texture.palette_words_raw[usize::from(*index)];
        pixels.extend([
            ((v & 31) << 3) as u8,
            (((v >> 5) & 31) << 3) as u8,
            (((v >> 10) & 31) << 3) as u8,
            if v & 0x8000 != 0 { 128 } else { 0 },
        ]);
    }
    Ok(RgbaImage {
        width: texture.width,
        height: texture.height,
        pixels,
    })
}

/// Everything a large face needs, decoded from the user's import: the
/// executable, MSG_E.DAT, TEXSYS.MRG and PICTURE.MRG.
pub struct FaceAssets {
    pub tables: FaceTables,
    pub ramp: [u8; 16],
    font: roses_assets::MessageFont,
    texsys: Vec<RgbaImage>,
    pictures: Vec<u8>,
    message_file: Vec<u8>,
    executable: Vec<u8>,
}

/// TEXSYS records the faces use (large frames #0..#3, small frames #4,
/// icons #6).
pub const FACE_TEXSYS_RECORDS: [u8; 6] = [0, 1, 2, 3, 4, 6];

impl FaceAssets {
    pub fn new(
        executable: Vec<u8>,
        message_file: Vec<u8>,
        texsys_archive: &[u8],
        pictures: Vec<u8>,
    ) -> Result<Self> {
        let tables = FaceTables::from_executable(&executable)?;
        let ramp = text_alpha_ramp(&executable)?;
        let font = roses_assets::decode_pal_name_font(&message_file).map_err(|e| e.to_string())?;
        let mut texsys = Vec::new();
        for record in 0..=*FACE_TEXSYS_RECORDS.iter().max().unwrap() {
            let texture = roses_assets::decode_pal_opening_indexed_texture(
                texsys_archive,
                roses_assets::OpeningArchiveKind::TexSys,
                usize::from(record),
            )
            .map_err(|e| e.to_string())?;
            texsys.push(indexed_gs_rgba(&texture)?);
        }
        Ok(Self {
            tables,
            ramp,
            font,
            texsys,
            pictures,
            message_file,
            executable,
        })
    }

    /// The face of `card` with the given values (ATK/DEF as the duel or the
    /// table has them; level as 0x1E1BD0). Off the board the effect frame
    /// follows the card table (0x1E1D88).
    pub fn face(
        &self,
        card: u16,
        level: u8,
        attack: i32,
        defense: i32,
        size: FaceSize,
    ) -> Result<RgbaImage> {
        self.face_with_effect(card, level, attack, defense, None, size)
    }

    /// [`FaceAssets::face`] with the effect frame decided by the caller (a
    /// field unit's 0x1E1C60 on the board); `None` uses the card table.
    pub fn face_with_effect(
        &self,
        card: u16,
        level: u8,
        attack: i32,
        defense: i32,
        effect: Option<bool>,
        size: FaceSize,
    ) -> Result<RgbaImage> {
        let ids = [
            DIGITS_MESSAGE,
            ATK_MESSAGE,
            DEF_MESSAGE,
            NAME_MESSAGE_BASE + card,
        ];
        let messages = roses_assets::parse_pal_messages(&self.message_file, &ids)
            .map_err(|e| e.to_string())?
            .messages;
        let mut page = blank_text_page();
        render_text_row(
            &mut page,
            &self.font,
            &messages[0].expanded_codes[..DIGIT_GLYPHS],
            0,
            &self.ramp,
        )?;
        render_text_row(
            &mut page,
            &self.font,
            &messages[1].expanded_codes,
            1,
            &self.ramp,
        )?;
        render_text_row(
            &mut page,
            &self.font,
            &messages[2].expanded_codes,
            2,
            &self.ramp,
        )?;
        let name_width = render_text_row(
            &mut page,
            &self.font,
            &messages[3].expanded_codes,
            NAME_ROW,
            &self.ramp,
        )?;
        let record = card_record(&self.executable, card)?;
        let info = FaceCard {
            card,
            record,
            level,
            attack,
            defense,
            effect: effect.unwrap_or_else(|| record.has_effect()),
            name_width,
        };
        let sprites = face_sprites(&info, &self.tables, size)?;
        let picture = roses_assets::decode_picture_gs(&self.pictures, usize::from(card))
            .map_err(|e| e.to_string())?;
        let lookup = |slot: u8| self.texsys.get(usize::from(slot));
        compose(
            &sprites,
            &FaceTextures {
                texsys: &lookup,
                picture: Some(&picture),
                text_page: &page,
            },
            size,
        )
    }
}
