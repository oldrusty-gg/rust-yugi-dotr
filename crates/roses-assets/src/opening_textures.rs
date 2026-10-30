//! Fingerprint-gated PAL RH2 texture archives. This is a bounded indexed-texture
//! variant, not an arbitrary PS2 DMA/GIF executor. Unknown header words and palette
//! values are retained. Archive indices identify records, not semantic UI roles.

use crate::{Error, Result, RgbaImage, PROFILE_ID};
use serde::Serialize;
use sha2::{Digest, Sha256};

pub const MAX_OPENING_ARCHIVE_BYTES: usize = 8 * 1024 * 1024;
const HEADER_BYTES: usize = 0x60;
const PAGE_BYTES: usize = 8192;
const PACKET_BYTES: usize = PAGE_BYTES + 0x60;
const MAX_RECORDS: usize = 64;
const MAX_SIDE: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OpeningArchiveKind {
    TexSys,
    TexEve,
    TexEtc,
    TexEff,
    TexAnm,
}
impl OpeningArchiveKind {
    pub fn iso_path(self) -> &'static str {
        match self {
            Self::TexSys => "DATA/TEXSYS.MRG",
            Self::TexEve => "DATA/TEXEVE.MRG",
            Self::TexEtc => "DATA/TEXETC.MRG",
            Self::TexEff => "DATA/TEXEFF.MRG",
            Self::TexAnm => "DATA/TEXANM.MRG",
        }
    }
    pub fn expected_sha256(self) -> &'static str {
        match self {
            Self::TexSys => "c3330608f94fcaea1940ca49a729e5fd3927d3e54701bad818f836993284733f",
            Self::TexEve => "dff07c9e66f0665e2b451c816a43bfab74d77f5427ce76af947e26c14825b20e",
            Self::TexEtc => "5f0890347e09941399ab5a9c4a7c02e2cb0c5e630d65963abe1f3627f9d0ced4",
            Self::TexEff => "3500384a115f8f7207b5ffdf9b793f3798363b4c7dcd40689d29a271a53b5209",
            Self::TexAnm => "26e9d5d5131620f0ff4dbcb4eb6aed313eac26204cf30b4151daf17b79e938c5",
        }
    }
    fn expected_size(self) -> usize {
        match self {
            Self::TexSys => 1_173_504,
            Self::TexEve => 4_714_496,
            Self::TexEtc => 698_368,
            Self::TexEff => 280_576,
            Self::TexAnm => 1_484_800,
        }
    }
    fn expected_count(self) -> usize {
        match self {
            Self::TexSys => 8,
            Self::TexEve => 27,
            Self::TexEtc => 5,
            Self::TexEff => 16,
            Self::TexAnm => 22,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OpeningPaletteFormat {
    Rgb5551,
    /// Stored RGBA lanes; alpha 128 is converted to output alpha 255.
    /// This conversion does not reconstruct the game's per-draw blend settings.
    Rgba8888Alpha128,
}

#[derive(Clone, Debug, Serialize)]
pub struct OpeningTextureRecord {
    pub id: u16,
    pub file_offset: u64,
    pub byte_len: u32,
    pub width: u32,
    pub height: u32,
    pub palette_format: OpeningPaletteFormat,
    /// Offsets below are relative to this RH2 record, including its header.
    pub palette_packet_offset: u32,
    pub pixel_packets_offset: u32,
    pub pixel_packet_count: u32,
    /// All original 0x60 header bytes, as little-endian words. Unused fields have
    /// no assigned semantics; the validated subset is documented in the decoder.
    pub raw_header_words: [u32; 24],
    /// Original 256 palette values before bit-3/4 index permutation or conversion.
    /// For RGB5551, the upper sixteen bits of each value are zero.
    pub palette_words_raw: Vec<u32>,
}

#[derive(Clone, Debug, Serialize)]
pub struct OpeningArchivePadding {
    pub file_offset: u64,
    pub byte_len: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct OpeningTextureArchive {
    pub schema_version: u32,
    pub profile_id: String,
    pub kind: OpeningArchiveKind,
    pub iso_path: String,
    pub archive_sha256: String,
    pub records: Vec<OpeningTextureRecord>,
    pub zero_padding: Vec<OpeningArchivePadding>,
    /// Decoding source assets is not validation of rendering/gameplay behavior.
    pub gameplay_validated: bool,
}

/// Lossless indexed pixels for renderers that implement the original GS color
/// arithmetic. Unlike an ordinary RGBA PNG, this preserves alpha values above
/// 128 and the original 16-bit palette words without expanding their channels.
#[derive(Clone, Debug)]
pub struct OpeningIndexedTexture {
    pub width: u32,
    pub height: u32,
    pub palette_format: OpeningPaletteFormat,
    pub palette_words_raw: Vec<u32>,
    /// Top-down row-major indices into `palette_words_raw`. Texture-page
    /// swizzling and the CLUT bit-3/4 permutation have already been removed.
    pub palette_indices: Vec<u8>,
}

/// Inspect a complete supported archive, requiring its exact source fingerprint.
/// The caller should obtain it from an inspected local ISO; no retail bytes are
/// embedded here. Record IDs follow archive order and are stable for this hash.
pub fn inspect_pal_opening_archive(
    archive: &[u8],
    kind: OpeningArchiveKind,
) -> Result<OpeningTextureArchive> {
    if archive.len() > MAX_OPENING_ARCHIVE_BYTES || archive.len() != kind.expected_size() {
        return Err(invalid("opening archive has an unsupported byte length"));
    }
    let archive_sha256 = format!("{:x}", Sha256::digest(archive));
    if archive_sha256 != kind.expected_sha256() {
        return Err(Error::Unsupported(format!(
            "{} SHA256 {archive_sha256}",
            kind.iso_path()
        )));
    }
    let (records, zero_padding) = inspect_records(archive)?;
    if records.len() != kind.expected_count() {
        return Err(invalid(
            "opening archive record count does not match profile",
        ));
    }
    Ok(OpeningTextureArchive {
        schema_version: 1,
        profile_id: PROFILE_ID.into(),
        kind,
        iso_path: kind.iso_path().into(),
        archive_sha256,
        records,
        zero_padding,
        gameplay_validated: false,
    })
}

/// Decode one indexed record into top-down, unpremultiplied RGBA. Uses a full
/// fingerprint and bounded structural validation on every call. The palette and
/// header remain available separately through `inspect_pal_opening_archive`.
pub fn decode_pal_opening_texture(
    archive: &[u8],
    kind: OpeningArchiveKind,
    index: usize,
) -> Result<RgbaImage> {
    let report = inspect_pal_opening_archive(archive, kind)?;
    let record = report.records.get(index).ok_or_else(|| {
        invalid(format!(
            "opening texture index {index} is outside {} records",
            report.records.len()
        ))
    })?;
    decode_record(archive, record)
}

/// Decode one original texture without normalizing its palette to display RGBA.
/// The same full-archive fingerprint and structural checks apply as for PNG
/// conversion. Draw-specific GS blending, TEXA and filtering remain the
/// renderer's responsibility; palette alpha is not conventional opacity.
pub fn decode_pal_opening_indexed_texture(
    archive: &[u8],
    kind: OpeningArchiveKind,
    index: usize,
) -> Result<OpeningIndexedTexture> {
    let report = inspect_pal_opening_archive(archive, kind)?;
    let record = report.records.get(index).ok_or_else(|| {
        invalid(format!(
            "opening texture index {index} is outside {} records",
            report.records.len()
        ))
    })?;
    decode_indexed_record(archive, record)
}

fn invalid(message: impl Into<String>) -> Error {
    Error::Invalid(message.into())
}
pub(crate) fn u32_at(bytes: &[u8], offset: usize) -> Result<u32> {
    let raw = bytes
        .get(
            offset
                ..offset
                    .checked_add(4)
                    .ok_or_else(|| invalid("offset overflow"))?,
        )
        .ok_or_else(|| invalid("truncated RH2 word"))?;
    Ok(u32::from_le_bytes(raw.try_into().unwrap()))
}
fn u64_at(bytes: &[u8], offset: usize) -> Result<u64> {
    let low = u64::from(u32_at(bytes, offset)?);
    let high = u64::from(u32_at(bytes, offset + 4)?);
    Ok(low | high << 32)
}

fn inspect_records(
    archive: &[u8],
) -> Result<(Vec<OpeningTextureRecord>, Vec<OpeningArchivePadding>)> {
    if archive.is_empty() || archive.len() > MAX_OPENING_ARCHIVE_BYTES {
        return Err(invalid("opening archive exceeds supported bounds"));
    }
    let mut records = Vec::new();
    let mut zero_padding = Vec::new();
    let mut offset = 0usize;
    while offset < archive.len() {
        if archive.get(offset..offset + 4) != Some(b"RH2\0") {
            let start = offset;
            while offset < archive.len() && archive[offset] == 0 && offset - start < 2048 {
                offset += 1;
            }
            if offset == start || offset - start >= 2048 || !offset.is_multiple_of(2048) {
                return Err(invalid(
                    "unknown data or invalid padding between RH2 records",
                ));
            }
            zero_padding.push(OpeningArchivePadding {
                file_offset: start as u64,
                byte_len: (offset - start) as u32,
            });
            continue;
        }
        if records.len() >= MAX_RECORDS {
            return Err(invalid("opening archive exceeds maximum record count"));
        }
        let size = u32_at(archive, offset + 8)? as usize;
        if size < HEADER_BYTES || !size.is_multiple_of(16) {
            return Err(invalid("RH2 record length is invalid"));
        }
        let end = offset
            .checked_add(size)
            .filter(|end| *end <= archive.len())
            .ok_or_else(|| invalid("RH2 record extends beyond archive"))?;
        let record = inspect_record(&archive[offset..end], records.len(), offset)?;
        records.push(record);
        offset = end;
    }
    if records.is_empty() {
        return Err(invalid("opening archive contains no records"));
    }
    Ok((records, zero_padding))
}

pub(crate) fn inspect_record(
    bytes: &[u8],
    id: usize,
    file_offset: usize,
) -> Result<OpeningTextureRecord> {
    if bytes.len() < HEADER_BYTES || bytes.get(..4) != Some(b"RH2\0") {
        return Err(invalid("RH2 header is missing or truncated"));
    }
    let mut header = [0u32; 24];
    for (i, word) in header.iter_mut().enumerate() {
        *word = u32_at(bytes, i * 4)?;
    }
    let width = (header[21] & 0xffff) as usize;
    let height = (header[21] >> 16) as usize;
    let (pixel_format, palette_format) = match header[20] {
        0x0002_0013 => (PixelFormat::P8, OpeningPaletteFormat::Rgb5551),
        0x0000_0013 => (PixelFormat::P8, OpeningPaletteFormat::Rgba8888Alpha128),
        0x0002_0014 => (PixelFormat::P4, OpeningPaletteFormat::Rgb5551),
        0x0000_0014 => (PixelFormat::P4, OpeningPaletteFormat::Rgba8888Alpha128),
        _ => return Err(Error::Unsupported("RH2 pixel/palette format".into())),
    };
    let layout = record_layout(bytes, width, height, pixel_format).ok_or_else(|| {
        invalid("RH2 dimensions must fit bounded 128-by-64 pixel pages or one small P_8 rectangle")
    })?;
    // `pages` counts pixel upload packets: whole 8 KiB pages, or the bands
    // of one small rectangle stacked from the top (observed: a 32x128
    // battle-stage texture in two 32x64 packets, a 64x256 model texture in
    // four 64x64 packets); the header's descriptor count says how many.
    let (pages, pixel_payload) = match layout {
        PixelLayout::Pages | PixelLayout::P4Pages => {
            (layout_pages(layout, width, height), PAGE_BYTES)
        }
        PixelLayout::SmallRect { bands } => (bands, width * height / bands),
    };
    let packet_bytes = HEADER_BYTES + pixel_payload;
    let palette_entry_bytes = match palette_format {
        OpeningPaletteFormat::Rgb5551 => 2,
        OpeningPaletteFormat::Rgba8888Alpha128 => 4,
    };
    // A P_4 texture's CLUT is 16 entries uploaded as one 8x2 rectangle.
    let (palette_entries, palette_w, palette_h) = match pixel_format {
        PixelFormat::P8 => (256, 16, 16),
        PixelFormat::P4 => (16, 8, 2),
    };
    let palette_size = HEADER_BYTES + palette_entries * palette_entry_bytes;
    let palette_start = HEADER_BYTES + (pages + 1) * 48;
    let pixels_start = palette_start + palette_size;
    let expected_size = pixels_start + pages * packet_bytes;
    if header[2] as usize != bytes.len()
        || header[4] != HEADER_BYTES as u32
        || header[5] as usize != (pages + 1) * 48
        || header[6] as usize != palette_start
        || header[7] as usize != palette_size
        || header[8] as usize != pixels_start
        || header[9] as usize != pages * packet_bytes
        || header[10] as usize != expected_size
        || header[22] as usize != pages + 1
        || bytes.len() != expected_size
    {
        return Err(invalid(
            "RH2 packet regions, dimensions and record length disagree",
        ));
    }
    // This decoder supports the observed fixed packet form. Validate input
    // offsets as data; do not follow arbitrary DMA chains or execute packets.
    for index in 0..=pages {
        let descriptor = HEADER_BYTES + index * 48;
        let data_offset = if index == 0 {
            palette_start
        } else {
            pixels_start + (index - 1) * packet_bytes
        };
        // Palette: C_16 or C_32. Pixels: pre-swizzled C_32 pages, or one
        // linear P_8 rectangle that the GS stores in its own P_8 layout.
        // The destination buffer width (64-pixel units) is 1 for the
        // CLUT and for 64x32 C_32 pages; a P_8 rectangle uses its own width
        // rounded up (2 for the 96- and 128-wide model/effect textures).
        let (destination_format, buffer_width) = match (index, palette_entry_bytes, layout) {
            (0, 2, _) => (2, 1),
            (0, _, _) | (_, _, PixelLayout::Pages | PixelLayout::P4Pages) => (0, 1),
            (_, _, PixelLayout::SmallRect { .. }) => (0x13, width.div_ceil(64) as u32),
        };
        for (relative, value) in [
            (0, 0x5000_0002),
            (4, data_offset as u32),
            (8, 0x1100_0000),
            (12, 0x5100_0002),
            (16, 0x0000_8001),
            (20, 0x1000_0000),
            (24, 0x0000_000e),
            (28, 0),
            (32, 0),
            (36, buffer_width << 16 | destination_format << 24),
            (40, 0x50),
            (44, 0),
        ] {
            if u32_at(bytes, descriptor + relative)? != value {
                return Err(Error::Unsupported("RH2 transfer descriptor layout".into()));
            }
        }
    }
    validate_packet(
        bytes,
        palette_start,
        palette_entries * palette_entry_bytes,
        palette_w,
        palette_h,
    )?;
    let (packet_width, packet_height) = match layout {
        PixelLayout::Pages | PixelLayout::P4Pages => (64, 32),
        PixelLayout::SmallRect { bands } => (width as u64, (height / bands) as u64),
    };
    for page in 0..pages {
        validate_packet(
            bytes,
            pixels_start + page * packet_bytes,
            pixel_payload,
            packet_width,
            packet_height,
        )?;
    }
    let palette_bytes = &bytes[palette_start + HEADER_BYTES..pixels_start];
    let palette_words_raw = palette_bytes
        .chunks_exact(palette_entry_bytes)
        .map(|value| {
            if palette_entry_bytes == 2 {
                u32::from(u16::from_le_bytes(value.try_into().unwrap()))
            } else {
                u32::from_le_bytes(value.try_into().unwrap())
            }
        })
        .collect();
    Ok(OpeningTextureRecord {
        id: id as u16,
        file_offset: file_offset as u64,
        byte_len: bytes.len() as u32,
        width: width as u32,
        height: height as u32,
        palette_format,
        palette_packet_offset: palette_start as u32,
        pixel_packets_offset: pixels_start as u32,
        pixel_packet_count: pages as u32,
        raw_header_words: header,
        palette_words_raw,
    })
}

/// Pixel storage of a supported RH2 record (PSM in header word 20).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PixelFormat {
    P8,
    P4,
}

/// Pixel upload form of a supported RH2 record, read from the first pixel
/// descriptor's BITBLTBUF destination format (as the GS receives it):
/// - C_32 (0): whole P_8 pages of 128x64 (P_4: 128x128), each sent as one
///   pre-swizzled 64x32 C_32 rectangle; a texture whose sides are not page
///   multiples occupies the pages that cover it (observed 192x64, 192x128,
///   256x112 effect textures in 2, 4 and 4 pages), row-major.
/// - P_8 (0x13): one linear P_8 rectangle, sent as `bands` equal packets
///   stacked from the top (64x64, 64x32, 128x32, 128x16, 96x64 in one,
///   64x128 in two and 64x256 in four 64x64 packets).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PixelLayout {
    Pages,
    P4Pages,
    SmallRect { bands: usize },
}

fn record_layout(
    bytes: &[u8],
    width: usize,
    height: usize,
    format: PixelFormat,
) -> Option<PixelLayout> {
    if width < 16 || height < 16 || width > MAX_SIDE || height > MAX_SIDE {
        return None;
    }
    let descriptors = (u32_at(bytes, 0x58).ok()? as usize).checked_sub(1)?;
    let destination = u32_at(bytes, HEADER_BYTES + 48 + 36).ok()? >> 24;
    match (format, destination) {
        (PixelFormat::P8, 0) => Some(PixelLayout::Pages),
        (PixelFormat::P4, 0) => Some(PixelLayout::P4Pages),
        (PixelFormat::P8, 0x13) => {
            let bands = descriptors.max(1);
            let fits = width.is_multiple_of(16)
                && height.is_multiple_of(bands)
                && (height / bands).is_multiple_of(16)
                && width * (height / bands) <= PAGE_BYTES;
            fits.then_some(PixelLayout::SmallRect { bands })
        }
        _ => None,
    }
}

fn layout_pages(layout: PixelLayout, width: usize, height: usize) -> usize {
    match layout {
        PixelLayout::Pages => width.div_ceil(128) * height.div_ceil(64),
        PixelLayout::P4Pages => width.div_ceil(128) * height.div_ceil(128),
        PixelLayout::SmallRect { bands } => bands,
    }
}

pub(crate) fn validate_packet(
    bytes: &[u8],
    offset: usize,
    payload: usize,
    w: u64,
    h: u64,
) -> Result<()> {
    let qwords = (payload / 16) as u64;
    for (relative, expected) in [
        (0, 0x6000_0000 | (qwords + 5)),
        (8, (0x5100_0000 | (qwords + 5)) << 32),
        (16, 0x1000_0000_0000_0003),
        (24, 0x0e),
        (32, 0),
        (40, 0x51),
        (48, w | h << 32),
        (56, 0x52),
        (64, 0),
        (72, 0x53),
        (80, 0x0800_0000_0000_8000 | qwords),
        (88, 0),
    ] {
        if u64_at(bytes, offset + relative)? != expected {
            return Err(Error::Unsupported("RH2 palette/image packet layout".into()));
        }
    }
    Ok(())
}

fn page_index(x: usize, y: usize) -> usize {
    let block = (y & !15) * 128 + (x & !15) * 2;
    let swap = (((y + 2) / 4) & 1) * 4;
    let row = (((y & !3) / 2) + (y & 1)) & 7;
    let column = row * 256 + ((x + swap) & 7) * 4;
    let lane = ((y / 2) & 1) + ((x / 4) & 2);
    block + column + lane
}
/// GS word of pixel (x, y) inside one 64x32 PSMCT32 page (block and
/// column arrangement of the GS manual).
fn c32_page_word(x: usize, y: usize) -> usize {
    const BLOCKS: [[usize; 8]; 4] = [
        [0, 1, 4, 5, 16, 17, 20, 21],
        [2, 3, 6, 7, 18, 19, 22, 23],
        [8, 9, 12, 13, 24, 25, 28, 29],
        [10, 11, 14, 15, 26, 27, 30, 31],
    ];
    BLOCKS[y / 8][x / 8] * 64 + ((y % 8) / 2) * 16 + COLUMN_WORDS[y % 2][x % 8]
}
const COLUMN_WORDS: [[usize; 8]; 2] = [[0, 1, 4, 5, 8, 9, 12, 13], [2, 3, 6, 7, 10, 11, 14, 15]];

/// Word within its 64-word block and the byte (P_8) or nibble-pair (P_4)
/// sub-lane of an indexed pixel; `bx` is the x inside the block, `by` < 16.
/// Rows 2-3 of even columns and rows 0-1 of odd ones are shifted by four.
fn indexed_block_position(bx: usize, by: usize) -> (usize, usize) {
    let (column, row) = (by / 4, by % 4);
    let swap = if (row >= 2) != (column % 2 == 1) { 4 } else { 0 };
    let word = column * 16 + COLUMN_WORDS[row % 2][(bx + swap) % 8];
    (word, 2 * (bx / 8) + usize::from(row >= 2))
}

/// Nibble offset, in the uploaded linear 64x32 C_32 data of one page, of
/// P_4 pixel (x, y) of its 128x128 page.
fn p4_page_nibble(x: usize, y: usize) -> usize {
    use std::sync::OnceLock;
    static TABLE: OnceLock<Vec<u16>> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut host_word = vec![0u16; 2048];
        for i in 0..2048 {
            host_word[c32_page_word(i % 64, i / 64)] = i as u16;
        }
        const BLOCKS: [[usize; 4]; 8] = [
            [0, 2, 8, 10],
            [1, 3, 9, 11],
            [4, 6, 12, 14],
            [5, 7, 13, 15],
            [16, 18, 24, 26],
            [17, 19, 25, 27],
            [20, 22, 28, 30],
            [21, 23, 29, 31],
        ];
        let mut table = vec![0u16; 128 * 128];
        for y in 0..128 {
            for x in 0..128 {
                let (word, nibble) = indexed_block_position(x % 32, y % 16);
                let gs_word = BLOCKS[y / 16][x / 32] * 64 + word;
                table[y * 128 + x] = host_word[gs_word] * 8 + nibble as u16;
            }
        }
        table
    });
    usize::from(table[y * 128 + x])
}

pub(crate) fn color(value: u32, format: OpeningPaletteFormat) -> [u8; 4] {
    match format {
        OpeningPaletteFormat::Rgb5551 => [
            ((value & 31) * 255 / 31) as u8,
            (((value >> 5) & 31) * 255 / 31) as u8,
            (((value >> 10) & 31) * 255 / 31) as u8,
            if value & 0x8000 != 0 { 255 } else { 0 },
        ],
        OpeningPaletteFormat::Rgba8888Alpha128 => [
            value as u8,
            (value >> 8) as u8,
            (value >> 16) as u8,
            (((value >> 24) * 255 / 128).min(255)) as u8,
        ],
    }
}
pub(crate) fn decode_record(archive: &[u8], record: &OpeningTextureRecord) -> Result<RgbaImage> {
    let indexed = decode_indexed_record(archive, record)?;
    let pixels = indexed
        .palette_indices
        .iter()
        .flat_map(|index| {
            color(
                indexed.palette_words_raw[usize::from(*index)],
                indexed.palette_format,
            )
        })
        .collect();
    Ok(RgbaImage {
        width: indexed.width,
        height: indexed.height,
        pixels,
    })
}

pub(crate) fn decode_indexed_record(
    archive: &[u8],
    record: &OpeningTextureRecord,
) -> Result<OpeningIndexedTexture> {
    let start =
        usize::try_from(record.file_offset).map_err(|_| invalid("record offset overflow"))?;
    let bytes = archive
        .get(start..start + record.byte_len as usize)
        .ok_or_else(|| invalid("texture lies outside archive"))?;
    let width = record.width as usize;
    let height = record.height as usize;
    let format = match record.raw_header_words[20] & 0xff {
        0x14 => PixelFormat::P4,
        _ => PixelFormat::P8,
    };
    let layout = record_layout(bytes, width, height, format)
        .ok_or_else(|| invalid("texture dimensions have no supported pixel layout"))?;
    let mut palette_indices = vec![0u8; width * height];
    for y in 0..height {
        for x in 0..width {
            let offset = match layout {
                PixelLayout::Pages => {
                    let page = (y / 64) * width.div_ceil(128) + x / 128;
                    record.pixel_packets_offset as usize
                        + page * PACKET_BYTES
                        + HEADER_BYTES
                        + page_index(x % 128, y % 64)
                }
                PixelLayout::P4Pages => {
                    let page = (y / 128) * width.div_ceil(128) + x / 128;
                    let nibble = p4_page_nibble(x % 128, y % 128);
                    let byte = *bytes
                        .get(
                            record.pixel_packets_offset as usize
                                + page * PACKET_BYTES
                                + HEADER_BYTES
                                + nibble / 2,
                        )
                        .ok_or_else(|| invalid("texture pixel outside record"))?;
                    // Even GS nibbles are the low half; a 16-entry CLUT has
                    // no bit-3/4 permutation.
                    palette_indices[y * width + x] = (byte >> (4 * (nibble & 1))) & 15;
                    continue;
                }
                PixelLayout::SmallRect { bands } => {
                    // Bands of height / packets rows, one packet each.
                    let band_height = height / bands;
                    let band = y / band_height;
                    record.pixel_packets_offset as usize
                        + band * (HEADER_BYTES + width * band_height)
                        + HEADER_BYTES
                        + (y % band_height) * width
                        + x
                }
            };
            let index = usize::from(
                *bytes
                    .get(offset)
                    .ok_or_else(|| invalid("texture pixel outside record"))?,
            );
            let palette_index = (index & !24) | ((index & 8) << 1) | ((index & 16) >> 1);
            palette_indices[y * width + x] = palette_index as u8;
        }
    }
    Ok(OpeningIndexedTexture {
        width: record.width,
        height: record.height,
        palette_format: record.palette_format,
        palette_words_raw: record.palette_words_raw.clone(),
        palette_indices,
    })
}

#[cfg(test)]
mod tests;
