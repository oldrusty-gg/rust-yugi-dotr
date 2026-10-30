//! MPEG-2 video decoding (ISO/IEC 13818-2), Main Profile: 4:2:0, I/P/B
//! pictures, frame and field picture structures, frame/field/16x8/dual-prime
//! motion compensation, field DCT, both VLC tables, both scans, linear and
//! non-linear quantiser scales, downloaded matrices and mismatch control.
//! MPEG-1 streams, scalable extensions and 4:2:2/4:4:4 are rejected.
//!
//! Input is the video elementary stream in any chunking; output is frames in
//! display order.
use crate::bits::Bits;
#[cfg(not(feature = "verify-int-idct"))]
use crate::idct::idct;
#[cfg(feature = "verify-int-idct")]
use crate::idct::idct_int as idct;
use crate::tables::*;
use crate::{invalid, unsupported, Error, Result};
use std::collections::VecDeque;

/// A decoded picture in display order, cropped to the coded size.
#[derive(Clone)]
pub struct Frame {
    pub width: usize,
    pub height: usize,
    /// Luma, `width * height` bytes.
    pub y: Vec<u8>,
    /// Chroma, `(width/2) * (height/2)` bytes each.
    pub cb: Vec<u8>,
    pub cr: Vec<u8>,
    /// b'I', b'P' or b'B'.
    pub picture_type: u8,
    pub temporal_reference: u16,
    /// 0-based position in display order.
    pub display_index: u64,
}

impl Frame {
    /// BT.601 limited-range YCbCr to RGBA (alpha 255), with MPEG-2 4:2:0
    /// chroma siting (co-sited horizontally, interstitial vertically) and
    /// bilinear chroma upsampling. `out` must hold `width * height * 4`.
    pub fn to_rgba(&self, out: &mut [u8]) {
        let (w, h) = (self.width, self.height);
        let (cw, ch) = (w / 2, h / 2);
        assert!(out.len() >= w * h * 4);
        // Vertically upsampled chroma rows, reused per luma row.
        let mut ub = vec![0i32; cw];
        let mut vb = vec![0i32; cw];
        for y in 0..h {
            // Luma row y sits at chroma row (y - 0.5) / 2.
            let k = y / 2;
            let (near, far) = if y % 2 == 0 {
                (k, k.saturating_sub(1))
            } else {
                (k, (k + 1).min(ch - 1))
            };
            for x in 0..cw {
                ub[x] = 3 * i32::from(self.cb[near * cw + x]) + i32::from(self.cb[far * cw + x]);
                vb[x] = 3 * i32::from(self.cr[near * cw + x]) + i32::from(self.cr[far * cw + x]);
            }
            let row = &mut out[y * w * 4..(y + 1) * w * 4];
            for x in 0..w {
                let cx = x / 2;
                // Values in units of 1/8 (vertical 1/4 times horizontal 1/2).
                let (u8x, v8x) = if x % 2 == 0 {
                    (2 * ub[cx], 2 * vb[cx])
                } else {
                    let nx = (cx + 1).min(cw - 1);
                    (ub[cx] + ub[nx], vb[cx] + vb[nx])
                };
                let yy = (i32::from(self.y[y * w + x]) - 16) * 76309; // 1.164 * 2^16
                let u = u8x - 1024; // (U - 128) * 8
                let v = v8x - 1024;
                // Coefficients * 2^16 / 8.
                let r = (yy + 13074 * v + (1 << 15)) >> 16; // 1.596
                let g = (yy - 3203 * u - 6660 * v + (1 << 15)) >> 16; // 0.391, 0.813
                let b = (yy + 16525 * u + (1 << 15)) >> 16; // 2.017
                let px = &mut row[x * 4..x * 4 + 4];
                px[0] = r.clamp(0, 255) as u8;
                px[1] = g.clamp(0, 255) as u8;
                px[2] = b.clamp(0, 255) as u8;
                px[3] = 255;
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct SequenceInfo {
    pub width: u32,
    pub height: u32,
    pub aspect_ratio_code: u8,
    pub frame_rate_code: u8,
    /// Frames per second as (numerator, denominator).
    pub frame_rate: (u32, u32),
    pub bit_rate_400: u32,
    pub profile_level: u8,
    pub progressive_sequence: bool,
    pub chroma_format: u8,
    pub low_delay: bool,
}

/// Counters over everything decoded so far.
#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub pictures: [u64; 3],
    pub frame_pictures: u64,
    pub field_pictures: u64,
    pub frame_pred_frame_dct: u64,
    pub intra_vlc_format: u64,
    pub alternate_scan: u64,
    pub q_scale_type: u64,
    pub concealment_mv: u64,
    pub progressive_frame: u64,
    pub repeat_first_field: u64,
    pub top_field_first: u64,
    pub intra_dc_precision: [u64; 4],
    pub macroblocks: u64,
    pub skipped: u64,
    pub intra_mbs: u64,
    pub field_dct: u64,
    pub motion_frame: u64,
    pub motion_field: u64,
    pub motion_16x8: u64,
    pub motion_dual_prime: u64,
    pub quant_matrix_extensions: u64,
    pub sequence_headers: u64,
    pub gops: u64,
    pub slices: u64,
    pub errors: u64,
    pub frames_out: u64,
}

#[derive(Clone)]
struct Sequence {
    info: SequenceInfo,
    mb_w: usize,
    mb_h: usize,
    intra_q: [u8; 64],
    non_intra_q: [u8; 64],
}

#[derive(Clone, Copy, Default)]
struct Picture {
    temporal_reference: u16,
    /// 1 = I, 2 = P, 3 = B.
    coding_type: u8,
    f_code: [[u8; 2]; 2],
    intra_dc_precision: u8,
    /// 1 top field, 2 bottom field, 3 frame.
    structure: u8,
    top_field_first: bool,
    frame_pred_frame_dct: bool,
    concealment_mv: bool,
    q_scale_type: bool,
    intra_vlc_format: bool,
    alternate_scan: bool,
    repeat_first_field: bool,
    progressive_frame: bool,
    has_extension: bool,
}

const STRUCT_TOP: u8 = 1;
const STRUCT_BOTTOM: u8 = 2;
const STRUCT_FRAME: u8 = 3;

#[derive(Clone)]
struct FrameBuf {
    y: Vec<u8>,
    cb: Vec<u8>,
    cr: Vec<u8>,
    coding_type: u8,
    temporal_reference: u16,
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
enum MotionKind {
    #[default]
    Frame,
    Field,
    DualPrime,
    Mc16x8,
}

#[derive(Clone, Copy, Default)]
struct Motion {
    fwd: bool,
    bwd: bool,
    kind: MotionKind,
    /// [r][s][t]: vector r (first/second), direction s (0 fwd, 1 bwd),
    /// component t (0 horizontal, 1 vertical); field vectors in field units.
    mv: [[[i32; 2]; 2]; 2],
    /// [r][s] reference field parity.
    field_select: [[u8; 2]; 2],
    dmv: [i32; 2],
}

/// Which frame buffers a picture predicts from.
#[derive(Clone, Copy)]
struct Refs {
    fwd: Option<usize>,
    bwd: Option<usize>,
    cur: usize,
    second_field: bool,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum PicState {
    None,
    HeaderParsed,
    Decoding,
}

pub struct VideoDecoder {
    es: Vec<u8>,
    scan_from: usize,
    seq: Option<Sequence>,
    seq_ext_seen: bool,
    pic: Picture,
    pic_state: PicState,
    frames: Vec<FrameBuf>,
    fwd: Option<usize>,
    bwd: Option<usize>,
    cur: usize,
    /// Parity of a first field waiting for its second field.
    pending_field: Option<u8>,
    second_field: bool,
    out: VecDeque<Frame>,
    display_index: u64,
    strict: bool,
    stats: Stats,
    last_error: Option<String>,
    // Per-macroblock scratch.
    pred_y: [u8; 256],
    pred_cb: [u8; 64],
    pred_cr: [u8; 64],
}

impl Default for VideoDecoder {
    fn default() -> Self {
        Self::new()
    }
}

fn frame_rate(code: u8) -> Option<(u32, u32)> {
    Some(match code {
        1 => (24000, 1001),
        2 => (24, 1),
        3 => (25, 1),
        4 => (30000, 1001),
        5 => (30, 1),
        6 => (50, 1),
        7 => (60000, 1001),
        8 => (60, 1),
        _ => return None,
    })
}

fn trace() -> bool {
    static T: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *T.get_or_init(|| std::env::var_os("ROSES_MOVIE_TRACE").is_some())
}

fn find_start_code(buf: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    while i + 3 <= buf.len() {
        let c = buf[i + 2];
        if c > 1 {
            i += 3;
        } else if c == 1 && buf[i] == 0 && buf[i + 1] == 0 {
            return Some(i);
        } else {
            i += 1;
        }
    }
    None
}

impl VideoDecoder {
    pub fn new() -> Self {
        Self {
            es: Vec::new(),
            scan_from: 0,
            seq: None,
            seq_ext_seen: false,
            pic: Picture::default(),
            pic_state: PicState::None,
            frames: Vec::new(),
            fwd: None,
            bwd: None,
            cur: 0,
            pending_field: None,
            second_field: false,
            out: VecDeque::new(),
            display_index: 0,
            strict: false,
            stats: Stats::default(),
            last_error: None,
            pred_y: [0; 256],
            pred_cb: [0; 64],
            pred_cr: [0; 64],
        }
    }

    /// In strict mode a damaged slice is an error; otherwise it is counted in
    /// `stats().errors`, the rest of the slice is left as predicted, and
    /// decoding continues (what a player wants).
    pub fn set_strict(&mut self, strict: bool) {
        self.strict = strict;
    }

    pub fn stats(&self) -> &Stats {
        &self.stats
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub fn sequence(&self) -> Option<&SequenceInfo> {
        self.seq.as_ref().map(|s| &s.info)
    }

    /// Feed elementary stream bytes. Frames completed by them become
    /// available from [`Self::next_frame`].
    pub fn push(&mut self, data: &[u8]) -> Result<()> {
        self.es.extend_from_slice(data);
        let Some(mut start) = find_start_code(&self.es, 0) else {
            // Keep the last two bytes in case a start code straddles chunks.
            let keep = self.es.len().min(3);
            self.es.drain(..self.es.len() - keep);
            self.scan_from = 0;
            return Ok(());
        };
        let mut from = self.scan_from.max(start + 4);
        let es = std::mem::take(&mut self.es);
        let mut result = Ok(());
        while let Some(next) = find_start_code(&es, from) {
            if let Err(e) = self.unit(&es[start..next]) {
                result = Err(e);
                start = next;
                break;
            }
            start = next;
            from = next + 4;
        }
        self.es = es;
        self.es.drain(..start);
        self.scan_from = self.es.len().saturating_sub(2);
        result
    }

    /// End of stream: decode the last unit and release held frames.
    pub fn flush(&mut self) -> Result<()> {
        let es = std::mem::take(&mut self.es);
        if let Some(start) = find_start_code(&es, 0) {
            self.unit(&es[start..])?;
        }
        self.end_picture()?;
        if let Some(b) = self.bwd.take() {
            self.emit(b);
        }
        self.fwd = None;
        Ok(())
    }

    pub fn next_frame(&mut self) -> Option<Frame> {
        self.out.pop_front()
    }

    pub fn pending_frames(&self) -> usize {
        self.out.len()
    }

    fn unit(&mut self, unit: &[u8]) -> Result<()> {
        if unit.len() < 4 {
            return Ok(());
        }
        let code = unit[3];
        let body = &unit[4..];
        match code {
            0x00 => {
                self.end_picture()?;
                self.picture_header(body)
            }
            0x01..=0xAF => self.slice(code, body),
            0xB3 => {
                self.end_picture()?;
                self.sequence_header(body)
            }
            0xB5 => self.extension(body),
            0xB7 => self.end_picture(),
            0xB8 => {
                self.end_picture()?;
                self.stats.gops += 1;
                Ok(())
            }
            _ => Ok(()), // user data and anything else
        }
    }

    fn sequence_header(&mut self, body: &[u8]) -> Result<()> {
        let mut b = Bits::new(body);
        let width = b.read(12);
        let height = b.read(12);
        let aspect = b.read(4) as u8;
        let rate_code = b.read(4) as u8;
        let bit_rate = b.read(18);
        b.read(1); // marker
        b.read(10); // vbv_buffer_size
        b.read(1); // constrained_parameters_flag
        let mut intra_q = DEFAULT_INTRA;
        if b.bit() {
            for &pos in &ZIGZAG {
                intra_q[pos as usize] = b.read(8) as u8;
            }
        }
        let mut non_intra_q = [16u8; 64];
        if b.bit() {
            for &pos in &ZIGZAG {
                non_intra_q[pos as usize] = b.read(8) as u8;
            }
        }
        if b.overrun() || width == 0 || height == 0 {
            return invalid("truncated or empty sequence header");
        }
        if intra_q.contains(&0) || non_intra_q.contains(&0) {
            return invalid("zero quantiser matrix entry");
        }
        let rate = frame_rate(rate_code).ok_or_else(|| {
            Error::Invalid(format!("frame_rate_code {rate_code}"))
        })?;
        let mb_w = (width as usize).div_ceil(16);
        let mb_h = (height as usize).div_ceil(16);
        let resized = self
            .seq
            .as_ref()
            .is_some_and(|s| (s.info.width, s.info.height) != (width, height));
        if resized {
            // A new size invalidates references.
            self.frames.clear();
            self.fwd = None;
            self.bwd = None;
        }
        let info = SequenceInfo {
            width,
            height,
            aspect_ratio_code: aspect,
            frame_rate_code: rate_code,
            frame_rate: rate,
            bit_rate_400: bit_rate,
            profile_level: self.seq.as_ref().map_or(0, |s| s.info.profile_level),
            progressive_sequence: self
                .seq
                .as_ref()
                .is_some_and(|s| s.info.progressive_sequence),
            chroma_format: 1,
            low_delay: false,
        };
        self.seq = Some(Sequence {
            info,
            mb_w,
            mb_h,
            intra_q,
            non_intra_q,
        });
        self.seq_ext_seen = false;
        self.stats.sequence_headers += 1;
        Ok(())
    }

    fn extension(&mut self, body: &[u8]) -> Result<()> {
        let mut b = Bits::new(body);
        let id = b.read(4);
        match id {
            1 => {
                let Some(seq) = self.seq.as_mut() else {
                    return invalid("sequence extension before sequence header");
                };
                seq.info.profile_level = b.read(8) as u8;
                seq.info.progressive_sequence = b.bit();
                seq.info.chroma_format = b.read(2) as u8;
                let hx = b.read(2);
                let vx = b.read(2);
                b.read(12); // bit_rate_extension
                b.read(1);
                b.read(8); // vbv_buffer_size_extension
                seq.info.low_delay = b.bit();
                let n = b.read(2) + 1;
                let d = b.read(5) + 1;
                if seq.info.chroma_format != 1 {
                    return unsupported(format!("chroma_format {}", seq.info.chroma_format));
                }
                if hx != 0 || vx != 0 {
                    return unsupported("picture larger than 4095 lines/columns");
                }
                let (rn, rd) = seq.info.frame_rate;
                seq.info.frame_rate = (rn * n, rd * d);
                if !seq.info.progressive_sequence && seq.mb_h % 2 == 1 {
                    // Interlaced: height in macroblock pairs.
                    seq.mb_h += 1;
                }
                self.seq_ext_seen = true;
            }
            3 => {
                let Some(seq) = self.seq.as_mut() else {
                    return invalid("quant matrix extension before sequence header");
                };
                if b.bit() {
                    for &pos in &ZIGZAG {
                        seq.intra_q[pos as usize] = b.read(8) as u8;
                    }
                }
                if b.bit() {
                    for &pos in &ZIGZAG {
                        seq.non_intra_q[pos as usize] = b.read(8) as u8;
                    }
                }
                // Chroma matrices are not used in 4:2:0.
                if seq.intra_q.contains(&0) || seq.non_intra_q.contains(&0) {
                    return invalid("zero quantiser matrix entry");
                }
                self.stats.quant_matrix_extensions += 1;
            }
            8 => {
                if self.pic_state != PicState::HeaderParsed {
                    return invalid("picture coding extension outside a picture header");
                }
                let p = &mut self.pic;
                p.f_code = [
                    [b.read(4) as u8, b.read(4) as u8],
                    [b.read(4) as u8, b.read(4) as u8],
                ];
                p.intra_dc_precision = b.read(2) as u8;
                p.structure = b.read(2) as u8;
                p.top_field_first = b.bit();
                p.frame_pred_frame_dct = b.bit();
                p.concealment_mv = b.bit();
                p.q_scale_type = b.bit();
                p.intra_vlc_format = b.bit();
                p.alternate_scan = b.bit();
                p.repeat_first_field = b.bit();
                b.read(1); // chroma_420_type
                p.progressive_frame = b.bit();
                p.has_extension = true;
                if p.structure == 0 {
                    return invalid("reserved picture_structure");
                }
            }
            2 | 7 => {} // sequence display, picture display: presentation only
            5 | 9 | 10 => return unsupported("scalable extension"),
            _ => {}
        }
        Ok(())
    }

    fn picture_header(&mut self, body: &[u8]) -> Result<()> {
        let mut b = Bits::new(body);
        let temporal_reference = b.read(10) as u16;
        let coding_type = b.read(3) as u8;
        b.read(16); // vbv_delay
        if !(1..=3).contains(&coding_type) {
            return unsupported(format!("picture_coding_type {coding_type}"));
        }
        self.pic = Picture {
            temporal_reference,
            coding_type,
            ..Picture::default()
        };
        self.pic_state = PicState::HeaderParsed;
        Ok(())
    }

    fn alloc_frame(&mut self) -> usize {
        let seq = self.seq.as_ref().unwrap();
        let (w, h) = (seq.mb_w * 16, seq.mb_h * 16);
        for i in 0..self.frames.len() {
            if Some(i) != self.fwd && Some(i) != self.bwd {
                return i;
            }
        }
        self.frames.push(FrameBuf {
            y: vec![16; w * h],
            cb: vec![128; w * h / 4],
            cr: vec![128; w * h / 4],
            coding_type: 0,
            temporal_reference: 0,
        });
        self.frames.len() - 1
    }

    fn begin_picture(&mut self) -> Result<()> {
        let Some(seq) = self.seq.as_ref() else {
            return invalid("slice before sequence header");
        };
        if !self.seq_ext_seen || !self.pic.has_extension {
            return unsupported("MPEG-1 video (no sequence/picture coding extension)");
        }
        let _ = seq;
        let p = self.pic;
        let field = p.structure != STRUCT_FRAME;
        let parity = if p.structure == STRUCT_BOTTOM { 1 } else { 0 };
        self.second_field = field && self.pending_field == Some(1 - parity);
        if !self.second_field {
            if p.coding_type == 3 {
                self.cur = self.alloc_frame();
            } else {
                self.fwd = self.bwd;
                self.bwd = None;
                self.cur = self.alloc_frame();
                self.bwd = Some(self.cur);
            }
            let f = &mut self.frames[self.cur];
            f.coding_type = p.coding_type;
            f.temporal_reference = p.temporal_reference;
        }
        self.pending_field = if field && !self.second_field {
            Some(parity)
        } else {
            None
        };
        let s = &mut self.stats;
        s.pictures[usize::from(p.coding_type - 1)] += 1;
        if field {
            s.field_pictures += 1;
        } else {
            s.frame_pictures += 1;
        }
        s.frame_pred_frame_dct += u64::from(p.frame_pred_frame_dct);
        s.intra_vlc_format += u64::from(p.intra_vlc_format);
        s.alternate_scan += u64::from(p.alternate_scan);
        s.q_scale_type += u64::from(p.q_scale_type);
        s.concealment_mv += u64::from(p.concealment_mv);
        s.progressive_frame += u64::from(p.progressive_frame);
        s.repeat_first_field += u64::from(p.repeat_first_field);
        s.top_field_first += u64::from(p.top_field_first);
        s.intra_dc_precision[usize::from(p.intra_dc_precision)] += 1;
        self.pic_state = PicState::Decoding;
        Ok(())
    }

    fn end_picture(&mut self) -> Result<()> {
        if self.pic_state != PicState::Decoding {
            self.pic_state = PicState::None;
            return Ok(());
        }
        self.pic_state = PicState::None;
        if self.pending_field.is_some() {
            return Ok(()); // wait for the second field
        }
        if self.pic.coding_type == 3 {
            self.emit(self.cur);
        } else if let Some(prev) = self.fwd {
            self.emit(prev);
        }
        Ok(())
    }

    fn emit(&mut self, index: usize) {
        let seq = self.seq.as_ref().unwrap();
        let (w, h) = (seq.info.width as usize, seq.info.height as usize);
        let stride = seq.mb_w * 16;
        let f = &self.frames[index];
        let crop = |plane: &[u8], pw: usize, ph: usize, ps: usize| -> Vec<u8> {
            if pw == ps {
                plane[..pw * ph].to_vec()
            } else {
                (0..ph).flat_map(|r| plane[r * ps..r * ps + pw].iter().copied()).collect()
            }
        };
        let frame = Frame {
            width: w,
            height: h,
            y: crop(&f.y, w, h, stride),
            cb: crop(&f.cb, w / 2, h / 2, stride / 2),
            cr: crop(&f.cr, w / 2, h / 2, stride / 2),
            picture_type: [b'I', b'P', b'B'][usize::from(f.coding_type.clamp(1, 3) - 1)],
            temporal_reference: f.temporal_reference,
            display_index: self.display_index,
        };
        self.display_index += 1;
        self.stats.frames_out += 1;
        self.out.push_back(frame);
    }

    fn slice(&mut self, code: u8, body: &[u8]) -> Result<()> {
        match self.pic_state {
            PicState::None => return Ok(()), // slice without a picture: ignore
            PicState::HeaderParsed => self.begin_picture()?,
            PicState::Decoding => {}
        }
        self.stats.slices += 1;
        let refs = Refs {
            fwd: self.fwd,
            bwd: self.bwd,
            cur: self.cur,
            second_field: self.second_field,
        };
        match self.decode_slice(code, body, refs) {
            Ok(()) => Ok(()),
            Err(e) => {
                self.stats.errors += 1;
                self.last_error = Some(e.to_string());
                if self.strict {
                    Err(e)
                } else {
                    Ok(())
                }
            }
        }
    }

    fn decode_slice(&mut self, code: u8, body: &[u8], refs: Refs) -> Result<()> {
        let seq = self.seq.clone().unwrap();
        let p = self.pic;
        let field_pic = p.structure != STRUCT_FRAME;
        let mb_rows = if field_pic { seq.mb_h / 2 } else { seq.mb_h };
        let row = usize::from(code) - 1;
        if row >= mb_rows {
            return invalid(format!("slice row {row} beyond picture"));
        }
        let t = tables();
        let mut b = Bits::new(body);
        let mut qcode = b.read(5);
        if qcode == 0 {
            return invalid("quantiser_scale_code 0");
        }
        if b.bit() {
            b.read(8); // intra_slice, reserved_bits
            while b.bit() {
                b.read(8);
            }
        }
        let dc_reset = 1i32 << (7 + p.intra_dc_precision);
        let mut dc_pred = [dc_reset; 3];
        let mut pmv = [[[0i32; 2]; 2]; 2];
        let mut addr = (row * seq.mb_w) as isize - 1;
        let total = (mb_rows * seq.mb_w) as isize;
        let mut first = true;
        let mut prev_motion = Motion::default();
        let mut prev_intra = false;
        let mut block = [0i32; 64];
        loop {
            // macroblock_address_increment
            let mut inc = 0isize;
            loop {
                let peek = b.peek(11);
                if peek == 0b0000_0001_000 {
                    b.skip(11);
                    inc += 33;
                } else if peek == 0b0000_0001_111 {
                    b.skip(11); // MPEG-1 stuffing, tolerated
                } else {
                    match t.mb_addr_inc.decode(&mut b) {
                        Some(v) => {
                            inc += v as isize;
                            break;
                        }
                        None => return invalid("bad macroblock_address_increment"),
                    }
                }
                if b.overrun() {
                    return invalid("slice overrun");
                }
            }
            if !first && inc > 1 {
                // Skipped macroblocks.
                if p.coding_type == 1 {
                    return invalid("skipped macroblock in I picture");
                }
                for k in 1..inc {
                    let skipped = addr + k;
                    if skipped >= total {
                        return invalid("skipped macroblocks beyond picture");
                    }
                    self.stats.skipped += 1;
                    let motion = if p.coding_type == 2 {
                        pmv = [[[0; 2]; 2]; 2];
                        let parity = if p.structure == STRUCT_BOTTOM { 1 } else { 0 };
                        Motion {
                            fwd: true,
                            kind: if field_pic { MotionKind::Field } else { MotionKind::Frame },
                            field_select: [[parity; 2]; 2],
                            ..Motion::default()
                        }
                    } else {
                        if prev_intra {
                            return invalid("skipped B macroblock after intra");
                        }
                        prev_motion
                    };
                    dc_pred = [dc_reset; 3];
                    let (mx, my) = (skipped as usize % seq.mb_w, skipped as usize / seq.mb_w);
                    self.predict(&seq, &p, refs, mx, my, &motion)?;
                    self.store_prediction(&seq, &p, refs.cur, mx, my);
                }
            }
            addr += inc;
            first = false;
            if addr >= total {
                return invalid("macroblock address beyond picture");
            }
            let (mx, my) = (addr as usize % seq.mb_w, addr as usize / seq.mb_w);
            self.stats.macroblocks += 1;

            // macroblock_modes
            let ty = t.mb_type[usize::from(p.coding_type - 1)]
                .decode(&mut b)
                .ok_or_else(|| Error::Invalid("bad macroblock_type".into()))?;
            let intra = ty & MB_INTRA != 0;
            let mut motion = Motion {
                fwd: ty & MB_FWD != 0,
                bwd: ty & MB_BWD != 0,
                ..Motion::default()
            };
            if motion.fwd || motion.bwd {
                if !field_pic {
                    motion.kind = if p.frame_pred_frame_dct {
                        MotionKind::Frame
                    } else {
                        match b.read(2) {
                            1 => MotionKind::Field,
                            2 => MotionKind::Frame,
                            3 => MotionKind::DualPrime,
                            _ => return invalid("reserved frame_motion_type"),
                        }
                    };
                } else {
                    motion.kind = match b.read(2) {
                        1 => MotionKind::Field,
                        2 => MotionKind::Mc16x8,
                        3 => MotionKind::DualPrime,
                        _ => return invalid("reserved field_motion_type"),
                    };
                }
            }
            let pattern = ty & MB_PATTERN != 0;
            let field_dct =
                !field_pic && !p.frame_pred_frame_dct && (intra || pattern) && b.bit();
            if ty & MB_QUANT != 0 {
                qcode = b.read(5);
                if qcode == 0 {
                    return invalid("quantiser_scale_code 0");
                }
            }
            let qscale = if p.q_scale_type {
                i32::from(NON_LINEAR_QSCALE[qcode as usize])
            } else {
                2 * qcode as i32
            };

            if intra {
                self.stats.intra_mbs += 1;
                if p.concealment_mv {
                    // Concealment vectors: decoded for predictor state only.
                    if field_pic {
                        b.read(1);
                    }
                    for tc in 0..2 {
                        let v = self.motion_component(&mut b, &p, 0, tc, pmv[0][0][tc])?;
                        pmv[0][0][tc] = v;
                        pmv[1][0][tc] = v;
                    }
                    b.read(1); // marker
                } else {
                    pmv = [[[0; 2]; 2]; 2];
                }
            } else {
                dc_pred = [dc_reset; 3];
                if p.coding_type == 2 && !motion.fwd {
                    // P "No MC": zero vector, same-parity / frame prediction.
                    pmv = [[[0; 2]; 2]; 2];
                    motion.fwd = true;
                    motion.kind = if field_pic { MotionKind::Field } else { MotionKind::Frame };
                    let parity = if p.structure == STRUCT_BOTTOM { 1 } else { 0 };
                    motion.field_select = [[parity; 2]; 2];
                } else {
                    for s in 0..2 {
                        let used = if s == 0 { motion.fwd } else { motion.bwd };
                        if used {
                            self.motion_vectors(&mut b, &p, s, &mut motion, &mut pmv)?;
                        }
                    }
                }
                match motion.kind {
                    MotionKind::Frame => self.stats.motion_frame += 1,
                    MotionKind::Field => self.stats.motion_field += 1,
                    MotionKind::Mc16x8 => self.stats.motion_16x8 += 1,
                    MotionKind::DualPrime => self.stats.motion_dual_prime += 1,
                }
            }

            let cbp = if pattern {
                t.cbp
                    .decode(&mut b)
                    .ok_or_else(|| Error::Invalid("bad coded_block_pattern".into()))?
            } else if intra {
                63
            } else {
                0
            };
            if field_dct {
                self.stats.field_dct += 1;
            }
            if trace() {
                eprintln!(
                    "pic {} mb {addr} ({mx},{my}) inc {inc} type {ty:05b} q {qcode} cbp {cbp} fwd {} bwd {} mv {:?} bit {}",
                    self.stats.pictures.iter().sum::<u64>(),
                    motion.fwd, motion.bwd, motion.mv, b.position()
                );
            }

            if !intra {
                self.predict(&seq, &p, refs, mx, my, &motion)?;
                self.store_prediction(&seq, &p, refs.cur, mx, my);
            }
            for i in 0..6 {
                if cbp & (1 << (5 - i)) == 0 {
                    continue;
                }
                block.fill(0);
                self.block(&mut b, &seq, &p, i, intra, qscale, &mut dc_pred, &mut block)?;
                idct(&mut block);
                self.put_block(&seq, &p, refs.cur, mx, my, i, field_dct, intra, &block);
            }
            if b.overrun() {
                return invalid("slice data overrun");
            }
            prev_intra = intra;
            prev_motion = motion;
            // End of slice: 23 zero bits (start of the next start code).
            if b.remaining() <= 0 || b.peek(23) == 0 {
                break;
            }
        }
        Ok(())
    }

    fn motion_component(
        &self,
        b: &mut Bits,
        p: &Picture,
        s: usize,
        t: usize,
        pred: i32,
    ) -> Result<i32> {
        let f_code = p.f_code[s][t];
        if !(1..=9).contains(&f_code) {
            return invalid(format!("motion vector with f_code {f_code}"));
        }
        let magnitude = tables()
            .motion_code
            .decode(b)
            .ok_or_else(|| Error::Invalid("bad motion_code".into()))?;
        let r_size = u32::from(f_code - 1);
        let delta = if magnitude == 0 {
            0
        } else {
            let negative = b.bit();
            let d = if r_size == 0 {
                magnitude
            } else {
                ((magnitude - 1) << r_size) + b.read(r_size) as i32 + 1
            };
            if negative {
                -d
            } else {
                d
            }
        };
        let f = 1i32 << r_size;
        let (low, high, range) = (-16 * f, 16 * f - 1, 32 * f);
        let mut v = pred + delta;
        if v < low {
            v += range;
        } else if v > high {
            v -= range;
        }
        Ok(v)
    }

    fn dmvector(b: &mut Bits) -> Result<i32> {
        tables()
            .dmvector
            .decode(b)
            .ok_or_else(|| Error::Invalid("bad dmvector".into()))
    }

    fn motion_vectors(
        &self,
        b: &mut Bits,
        p: &Picture,
        s: usize,
        m: &mut Motion,
        pmv: &mut [[[i32; 2]; 2]; 2],
    ) -> Result<()> {
        let field_pic = p.structure != STRUCT_FRAME;
        match (field_pic, m.kind) {
            (false, MotionKind::Frame) => {
                for t in 0..2 {
                    let v = self.motion_component(b, p, s, t, pmv[0][s][t])?;
                    pmv[0][s][t] = v;
                    pmv[1][s][t] = v;
                    m.mv[0][s][t] = v;
                }
            }
            (false, MotionKind::Field) => {
                for r in 0..2 {
                    m.field_select[r][s] = b.read(1) as u8;
                    let x = self.motion_component(b, p, s, 0, pmv[r][s][0])?;
                    let y = self.motion_component(b, p, s, 1, pmv[r][s][1] >> 1)?;
                    pmv[r][s] = [x, y * 2];
                    m.mv[r][s] = [x, y];
                }
            }
            (false, MotionKind::DualPrime) => {
                if s != 0 || p.coding_type != 2 {
                    return invalid("dual prime outside a forward P prediction");
                }
                let x = self.motion_component(b, p, s, 0, pmv[0][s][0])?;
                let dx = Self::dmvector(b)?;
                let y = self.motion_component(b, p, s, 1, pmv[0][s][1] >> 1)?;
                let dy = Self::dmvector(b)?;
                pmv[0][s] = [x, y * 2];
                pmv[1][s] = [x, y * 2];
                m.mv[0][s] = [x, y];
                m.dmv = [dx, dy];
            }
            (true, MotionKind::Field) => {
                m.field_select[0][s] = b.read(1) as u8;
                for t in 0..2 {
                    let v = self.motion_component(b, p, s, t, pmv[0][s][t])?;
                    pmv[0][s][t] = v;
                    pmv[1][s][t] = v;
                    m.mv[0][s][t] = v;
                }
            }
            (true, MotionKind::Mc16x8) => {
                for r in 0..2 {
                    m.field_select[r][s] = b.read(1) as u8;
                    for t in 0..2 {
                        let v = self.motion_component(b, p, s, t, pmv[r][s][t])?;
                        pmv[r][s][t] = v;
                        m.mv[r][s][t] = v;
                    }
                }
            }
            (true, MotionKind::DualPrime) => {
                if s != 0 || p.coding_type != 2 {
                    return invalid("dual prime outside a forward P prediction");
                }
                let x = self.motion_component(b, p, s, 0, pmv[0][s][0])?;
                let dx = Self::dmvector(b)?;
                let y = self.motion_component(b, p, s, 1, pmv[0][s][1])?;
                let dy = Self::dmvector(b)?;
                pmv[0][s] = [x, y];
                pmv[1][s] = [x, y];
                m.mv[0][s] = [x, y];
                m.dmv = [dx, dy];
            }
            (false, MotionKind::Mc16x8) | (true, MotionKind::Frame) => {
                return invalid("motion type not allowed in this picture structure")
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn block(
        &self,
        b: &mut Bits,
        seq: &Sequence,
        p: &Picture,
        index: usize,
        intra: bool,
        qscale: i32,
        dc_pred: &mut [i32; 3],
        out: &mut [i32; 64],
    ) -> Result<()> {
        let t = tables();
        let scan = if p.alternate_scan { &ALTERNATE } else { &ZIGZAG };
        let mut sum = 0i32;
        let mut i;
        let (matrix, table) = if intra {
            let cc = if index < 4 { 0 } else { index - 3 };
            let size = if cc == 0 { &t.dc_luma } else { &t.dc_chroma }
                .decode(b)
                .ok_or_else(|| Error::Invalid("bad dct_dc_size".into()))?;
            let diff = if size == 0 {
                0
            } else {
                let raw = b.read(size as u32) as i32;
                if raw & (1 << (size - 1)) == 0 {
                    raw - (1 << size) + 1
                } else {
                    raw
                }
            };
            dc_pred[cc] += diff;
            let dc = dc_pred[cc] * (8 >> p.intra_dc_precision);
            if !(0..=2047).contains(&dc) {
                return invalid("intra DC out of range");
            }
            out[0] = dc;
            sum = dc;
            i = 1;
            (
                &seq.intra_q,
                if p.intra_vlc_format { &t.dct_b15 } else { &t.dct_b14 },
            )
        } else {
            i = 0;
            (&seq.non_intra_q, &t.dct_b14)
        };
        loop {
            let (run, level);
            if !intra && i == 0 && b.peek(1) == 1 {
                b.skip(1);
                run = 0;
                level = if b.bit() { -1 } else { 1 };
            } else {
                let v = table
                    .decode(b)
                    .ok_or_else(|| Error::Invalid("bad DCT coefficient code".into()))?;
                if v == DCT_EOB {
                    break;
                }
                if v == DCT_ESCAPE {
                    run = b.read(6) as usize;
                    let raw = b.read(12) as i32;
                    let l = (raw << 20) >> 20;
                    if l == 0 || l == -2048 {
                        return invalid("forbidden escape level");
                    }
                    level = l;
                } else {
                    run = (v & 0xFF) as usize;
                    let l = v >> 8;
                    level = if b.bit() { -l } else { l };
                }
            }
            i += run;
            if i > 63 {
                return invalid("DCT coefficient index beyond 63");
            }
            let pos = scan[i] as usize;
            let w = i32::from(matrix[pos]);
            let value = if intra {
                (2 * level * w * qscale) / 32
            } else {
                ((2 * level + level.signum()) * w * qscale) / 32
            }
            .clamp(-2048, 2047);
            out[pos] = value;
            sum += value;
            i += 1;
            if b.overrun() {
                return invalid("block overrun");
            }
        }
        if sum & 1 == 0 {
            out[63] ^= 1;
        }
        Ok(())
    }

    /// Plane geometry of the current picture: (offset of row 0, row step).
    fn picture_view(p: &Picture, stride: usize) -> (usize, usize) {
        match p.structure {
            STRUCT_TOP => (0, stride * 2),
            STRUCT_BOTTOM => (stride, stride * 2),
            _ => (0, stride),
        }
    }

    fn store_prediction(&mut self, seq: &Sequence, p: &Picture, cur: usize, mx: usize, my: usize) {
        let stride = seq.mb_w * 16;
        let (off, step) = Self::picture_view(p, stride);
        let f = &mut self.frames[cur];
        for r in 0..16 {
            let at = off + (my * 16 + r) * step + mx * 16;
            f.y[at..at + 16].copy_from_slice(&self.pred_y[r * 16..r * 16 + 16]);
        }
        let (coff, cstep) = Self::picture_view(p, stride / 2);
        for r in 0..8 {
            let at = coff + (my * 8 + r) * cstep + mx * 8;
            f.cb[at..at + 8].copy_from_slice(&self.pred_cb[r * 8..r * 8 + 8]);
            f.cr[at..at + 8].copy_from_slice(&self.pred_cr[r * 8..r * 8 + 8]);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn put_block(
        &mut self,
        seq: &Sequence,
        p: &Picture,
        cur: usize,
        mx: usize,
        my: usize,
        index: usize,
        field_dct: bool,
        intra: bool,
        block: &[i32; 64],
    ) {
        let stride = seq.mb_w * 16;
        let f = &mut self.frames[cur];
        let (plane, base, step) = if index < 4 {
            let (off, step) = Self::picture_view(p, stride);
            let bx = mx * 16 + (index & 1) * 8;
            if field_dct {
                // Field DCT (frame pictures only): blocks 0/1 hold the top
                // field's lines, 2/3 the bottom field's.
                (&mut f.y, off + (my * 16 + (index >> 1)) * step + bx, step * 2)
            } else {
                (&mut f.y, off + (my * 16 + (index >> 1) * 8) * step + bx, step)
            }
        } else {
            let (off, step) = Self::picture_view(p, stride / 2);
            let base = off + my * 8 * step + mx * 8;
            (if index == 4 { &mut f.cb } else { &mut f.cr }, base, step)
        };
        for r in 0..8 {
            let row = &mut plane[base + r * step..base + r * step + 8];
            let src = &block[r * 8..r * 8 + 8];
            if intra {
                for c in 0..8 {
                    row[c] = src[c].clamp(0, 255) as u8;
                }
            } else {
                for c in 0..8 {
                    row[c] = (i32::from(row[c]) + src[c]).clamp(0, 255) as u8;
                }
            }
        }
    }

    /// Form the macroblock's prediction into pred_y/pred_cb/pred_cr.
    fn predict(
        &mut self,
        seq: &Sequence,
        p: &Picture,
        refs: Refs,
        mx: usize,
        my: usize,
        m: &Motion,
    ) -> Result<()> {
        let stride = seq.mb_w * 16;
        let height = seq.mb_h * 16;
        let field_pic = p.structure != STRUCT_FRAME;
        let cur_parity = if p.structure == STRUCT_BOTTOM { 1u8 } else { 0 };
        let mut first = true;
        for s in 0..2 {
            let used = if s == 0 { m.fwd } else { m.bwd };
            if !used {
                continue;
            }
            // Forward prediction uses the previous anchor (for P pictures the
            // newest anchor is the picture being decoded); backward the newest.
            let anchor = if s == 0 { refs.fwd } else { refs.bwd };
            let field_ref = |parity: u8| -> Option<usize> {
                if field_pic && p.coding_type == 2 && refs.second_field && parity != cur_parity {
                    Some(refs.cur)
                } else {
                    anchor
                }
            };
            let avg = !first;
            let mut jobs: Vec<Job> = Vec::with_capacity(4);
            match (field_pic, m.kind) {
                (false, MotionKind::Frame) => {
                    jobs.push(Job::frame(anchor, m.mv[0][s], 0, 16, avg));
                }
                (false, MotionKind::Field) => {
                    for r in 0..2u8 {
                        jobs.push(Job::field_in_frame(
                            anchor,
                            m.field_select[usize::from(r)][s],
                            r,
                            m.mv[usize::from(r)][s],
                            avg,
                        ));
                    }
                }
                (false, MotionKind::DualPrime) => {
                    let [vx, vy] = m.mv[0][0];
                    for r in 0..2u8 {
                        // Same parity with the decoded vector.
                        jobs.push(Job::field_in_frame(anchor, r, r, [vx, vy], false));
                        // Opposite parity with the derived vector (Table 7-11/12).
                        let mul = if (r == 0) == p.top_field_first { 1 } else { 3 };
                        let e = if r == 0 { -1 } else { 1 };
                        let dx = ((vx * mul + i32::from(vx > 0)) >> 1) + m.dmv[0];
                        let dy = ((vy * mul + i32::from(vy > 0)) >> 1) + e + m.dmv[1];
                        jobs.push(Job::field_in_frame(anchor, 1 - r, r, [dx, dy], true));
                    }
                }
                (true, MotionKind::Field) => {
                    let fs = m.field_select[0][s];
                    jobs.push(Job::field(field_ref(fs), fs, 0, 16, m.mv[0][s], avg));
                }
                (true, MotionKind::Mc16x8) => {
                    for r in 0..2u8 {
                        let fs = m.field_select[usize::from(r)][s];
                        jobs.push(Job::field(field_ref(fs), fs, r * 8, 8, m.mv[usize::from(r)][s], avg));
                    }
                }
                (true, MotionKind::DualPrime) => {
                    let [vx, vy] = m.mv[0][0];
                    jobs.push(Job::field(field_ref(cur_parity), cur_parity, 0, 16, [vx, vy], false));
                    let e = if cur_parity == 0 { -1 } else { 1 };
                    let dx = ((vx + i32::from(vx > 0)) >> 1) + m.dmv[0];
                    let dy = ((vy + i32::from(vy > 0)) >> 1) + e + m.dmv[1];
                    let opp = 1 - cur_parity;
                    jobs.push(Job::field(field_ref(opp), opp, 0, 16, [dx, dy], true));
                }
                (false, MotionKind::Mc16x8) | (true, MotionKind::Frame) => {
                    return invalid("motion type not allowed in this picture structure")
                }
            }
            for job in jobs {
                let Some(src) = job.src else {
                    // Missing reference (e.g. open GOP at stream start): grey.
                    if !job.avg {
                        self.pred_y.fill(128);
                        self.pred_cb.fill(128);
                        self.pred_cr.fill(128);
                    }
                    continue;
                };
                let f = &self.frames[src];
                // Luma.
                let (loff, lstep, lh) = if job.field_parity.is_some() {
                    (usize::from(job.field_parity.unwrap()) * stride, stride * 2, height / 2)
                } else {
                    (0, stride, height)
                };
                let [vx, vy] = job.mv;
                let (dst_off, dst_step) = job.dst_rows(16);
                mc(
                    &mut self.pred_y,
                    dst_off,
                    dst_step,
                    &f.y,
                    loff,
                    lstep,
                    stride,
                    lh,
                    (mx * 16) as i32 + (vx >> 1),
                    job.base_y(my, 16) + (vy >> 1),
                    vx & 1 != 0,
                    vy & 1 != 0,
                    16,
                    job.rows,
                    job.avg,
                );
                // Chroma: vectors halved toward zero.
                let (cx, cy) = (vx / 2, vy / 2);
                let cstride = stride / 2;
                let (coff, cstep, chh) = if job.field_parity.is_some() {
                    (usize::from(job.field_parity.unwrap()) * cstride, cstride * 2, height / 4)
                } else {
                    (0, cstride, height / 2)
                };
                let (cdst_off, cdst_step) = job.dst_rows(8);
                for (dst, plane) in [(&mut self.pred_cb, &f.cb), (&mut self.pred_cr, &f.cr)] {
                    mc(
                        dst,
                        cdst_off,
                        cdst_step,
                        plane,
                        coff,
                        cstep,
                        cstride,
                        chh,
                        (mx * 8) as i32 + (cx >> 1),
                        job.base_y(my, 8) + (cy >> 1),
                        cx & 1 != 0,
                        cy & 1 != 0,
                        8,
                        job.rows / 2,
                        job.avg,
                    );
                }
            }
            first = false;
        }
        if first {
            return invalid("non-intra macroblock without prediction");
        }
        Ok(())
    }
}

/// One rectangular prediction into the macroblock prediction buffers.
struct Job {
    src: Option<usize>,
    /// Some(parity) to read one field of the reference.
    field_parity: Option<u8>,
    /// Destination layout: frame MB rows, or one field of a frame MB, or a
    /// row range of a field MB.
    dest: Dest,
    /// Luma rows produced.
    rows: usize,
    mv: [i32; 2],
    avg: bool,
}

#[derive(Clone, Copy)]
enum Dest {
    /// Rows `first..first+rows` of the MB, consecutive.
    Rows(u8),
    /// The given field (alternate rows) of a frame MB.
    FieldOfFrame(u8),
}

impl Job {
    fn frame(src: Option<usize>, mv: [i32; 2], first: u8, rows: usize, avg: bool) -> Self {
        Self { src, field_parity: None, dest: Dest::Rows(first), rows, mv, avg }
    }
    fn field_in_frame(src: Option<usize>, parity: u8, dest_field: u8, mv: [i32; 2], avg: bool) -> Self {
        Self { src, field_parity: Some(parity), dest: Dest::FieldOfFrame(dest_field), rows: 8, mv, avg }
    }
    fn field(src: Option<usize>, parity: u8, first: u8, rows: usize, mv: [i32; 2], avg: bool) -> Self {
        Self { src, field_parity: Some(parity), dest: Dest::Rows(first), rows, mv, avg }
    }
    /// (offset, row step) into a `width`-wide prediction buffer.
    fn dst_rows(&self, width: usize) -> (usize, usize) {
        match self.dest {
            Dest::Rows(first) => (usize::from(first) * width / 16 * width, width),
            Dest::FieldOfFrame(f) => (usize::from(f) * width, width * 2),
        }
    }
    /// Top row of the source area (before the vector) in the reference
    /// view's line coordinates, for a plane whose macroblock is `mb` lines.
    fn base_y(&self, my: usize, mb: usize) -> i32 {
        match self.dest {
            // Field-of-frame: the MB's lines in one field start at my*mb/2.
            Dest::FieldOfFrame(_) => (my * mb / 2) as i32,
            Dest::Rows(first) => {
                let first = usize::from(first) * mb / 16;
                (my * mb + first) as i32
            }
        }
    }
}

/// Half-sample motion compensation of a `w`x`h` block. The reference view
/// is `plane[off + line*step + x]` with `lines` lines of `width` samples;
/// positions outside are clamped to the edge.
#[allow(clippy::too_many_arguments)]
#[inline]
fn mc(
    dst: &mut [u8],
    dst_off: usize,
    dst_step: usize,
    plane: &[u8],
    off: usize,
    step: usize,
    width: usize,
    lines: usize,
    x: i32,
    y: i32,
    hx: bool,
    hy: bool,
    w: usize,
    h: usize,
    avg: bool,
) {
    let inside = x >= 0
        && y >= 0
        && (x as usize) + w + usize::from(hx) <= width
        && (y as usize) + h + usize::from(hy) <= lines;
    let max_x = width as i32 - 1;
    let max_y = lines as i32 - 1;
    let at = |xx: i32, yy: i32| -> i32 {
        if inside {
            i32::from(plane[off + yy as usize * step + xx as usize])
        } else {
            i32::from(plane[off + yy.clamp(0, max_y) as usize * step + xx.clamp(0, max_x) as usize])
        }
    };
    for j in 0..h {
        let yy = y + j as i32;
        let d = &mut dst[dst_off + j * dst_step..dst_off + j * dst_step + w];
        for (i, out) in d.iter_mut().enumerate() {
            let xx = x + i as i32;
            let v = match (hx, hy) {
                (false, false) => at(xx, yy),
                (true, false) => (at(xx, yy) + at(xx + 1, yy) + 1) >> 1,
                (false, true) => (at(xx, yy) + at(xx, yy + 1) + 1) >> 1,
                (true, true) => {
                    (at(xx, yy) + at(xx + 1, yy) + at(xx, yy + 1) + at(xx + 1, yy + 1) + 2) >> 2
                }
            };
            *out = if avg {
                ((i32::from(*out) + v + 1) >> 1) as u8
            } else {
                v as u8
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal MSB-first bit writer for hand-built test streams.
    #[derive(Default)]
    struct W {
        bytes: Vec<u8>,
        acc: u64,
        n: u32,
    }
    impl W {
        fn bits(&mut self, value: u32, count: u32) -> &mut Self {
            for i in (0..count).rev() {
                self.acc = self.acc << 1 | u64::from((value >> i) & 1);
                self.n += 1;
                if self.n == 8 {
                    self.bytes.push(self.acc as u8);
                    self.acc = 0;
                    self.n = 0;
                }
            }
            self
        }
        fn s(&mut self, code: &str) -> &mut Self {
            for c in code.chars().filter(|c| !c.is_whitespace()) {
                self.bits(u32::from(c == '1'), 1);
            }
            self
        }
        fn start(&mut self, code: u8) -> &mut Self {
            self.align();
            self.bytes.extend_from_slice(&[0, 0, 1, code]);
            self
        }
        fn align(&mut self) {
            while self.n != 0 {
                self.bits(0, 1);
            }
        }
    }

    /// 32x16 (two macroblocks), progressive, frame pictures.
    fn sequence(w: &mut W) {
        w.start(0xB3).bits(32, 12).bits(16, 12).bits(1, 4).bits(3, 4);
        w.bits(1000, 18).bits(1, 1).bits(10, 10).bits(0, 1).bits(0, 1).bits(0, 1);
        w.start(0xB5).bits(1, 4).bits(0x48, 8).bits(1, 1).bits(1, 2).bits(0, 4);
        w.bits(0, 12).bits(1, 1).bits(0, 8).bits(0, 1).bits(0, 2).bits(0, 5);
    }

    fn picture(w: &mut W, tref: u32, ty: u32, f: [[u32; 2]; 2]) {
        w.start(0x00).bits(tref, 10).bits(ty, 3).bits(0xFFFF, 16);
        if ty >= 2 {
            w.bits(0, 1).bits(7, 3);
        }
        if ty == 3 {
            w.bits(0, 1).bits(7, 3);
        }
        w.bits(0, 1);
        w.start(0xB5).bits(8, 4);
        for s in f {
            for t in s {
                w.bits(t, 4);
            }
        }
        // dc precision 0, frame, tff 0, frame_pred_frame_dct 1, cmv 0,
        // q_scale_type 0, intra_vlc 0, alternate 0, rff 0, 420 1, progressive 1, composite 0
        w.bits(0, 2).bits(3, 2).s("0 1 0 0 0 0 0 1 1 0");
        w.start(0x01).bits(8, 5).bits(0, 1);
    }

    /// Intra MB with luma DC differential `luma` on block 0 (others 0) and
    /// chroma differentials 0; intra_vlc_format 0 so EOB is '10'.
    fn intra_mb(w: &mut W, luma_size_code: &str, diff_bits: (u32, u32)) {
        w.s("1").s("1"); // address increment 1, I type intra
        w.s(luma_size_code).bits(diff_bits.0, diff_bits.1).s("10");
        for _ in 1..4 {
            w.s("100").s("10"); // dc size 0, EOB
        }
        for _ in 0..2 {
            w.s("00").s("10"); // chroma dc size 0, EOB
        }
    }

    fn decode(stream: &[u8], chunk: usize) -> Vec<Frame> {
        let mut d = VideoDecoder::new();
        d.set_strict(true);
        let mut out = Vec::new();
        for c in stream.chunks(chunk) {
            d.push(c).unwrap();
            while let Some(f) = d.next_frame() {
                out.push(f);
            }
        }
        d.flush().unwrap();
        while let Some(f) = d.next_frame() {
            out.push(f);
        }
        assert_eq!(d.stats().errors, 0);
        out
    }

    #[test]
    fn intra_forward_half_pel_and_bidirectional_reordering() {
        let mut w = W::default();
        sequence(&mut w);
        // I: MB0 luma 128-28=100 (size 5, diff -28 -> raw 3), MB1 228 -> 200
        // (pred continues: 100 + 100 = 200, size 7, raw 100).
        picture(&mut w, 0, 1, [[15; 2]; 2]);
        intra_mb(&mut w, "1110", (3, 5));
        intra_mb(&mut w, "1111 10", (100, 7));
        // P (f_code 3): MB0 vector +32 half-pels = MB1's position;
        // MB1 vector -1 (half-pel left of itself). Both MC, not coded.
        picture(&mut w, 3, 2, [[3, 3], [15, 15]]);
        w.s("1").s("001").s("0000 0101 1").s("0").bits(3, 2).s("1");
        w.s("1").s("001").s("0000 0101 0").s("1").bits(0, 2).s("1");
        // B: MB0 forward zero vector (I), MB1 backward zero vector (P).
        picture(&mut w, 1, 3, [[1, 1], [1, 1]]);
        w.s("1").s("0010").s("1").s("1");
        w.s("1").s("010").s("1").s("1");
        w.start(0xB7);
        let stream = w.bytes.clone();
        for chunk in [1, 3, 64, 1 << 20] {
            let frames = decode(&stream, chunk);
            assert_eq!(frames.len(), 3, "chunk {chunk}");
            let types: Vec<u8> = frames.iter().map(|f| f.picture_type).collect();
            assert_eq!(types, b"IBP");
            let (i, b, p) = (&frames[0], &frames[1], &frames[2]);
            // I: block 0 holds the differential, blocks 1-3 repeat the predictor.
            assert!(i.y[0..8].iter().all(|&v| v == 100));
            assert!(i.y[16..32].iter().all(|&v| v == 200));
            assert!(i.cb.iter().all(|&v| v == 128));
            // P MB0 copies the I picture's MB1.
            assert!(p.y[0..16].iter().all(|&v| v == 200));
            // P MB1: half-pel between x=15 (100) and x=16 (200) then 200s.
            assert_eq!(p.y[16], 150);
            assert!(p.y[17..32].iter().all(|&v| v == 200));
            // B: MB0 from I (100), MB1 from P (150, 200...).
            assert!(b.y[0..16].iter().all(|&v| v == 100));
            assert_eq!(&b.y[16..18], &[150, 200]);
            assert_eq!(b.display_index, 1);
        }
    }

    #[test]
    fn damaged_slices_are_errors_in_strict_mode_and_concealed_otherwise() {
        let mut w = W::default();
        sequence(&mut w);
        picture(&mut w, 0, 1, [[15; 2]; 2]);
        w.s("1").s("0000 0000 01"); // address increment then an invalid type
        w.bits(0xFF, 8);
        w.start(0xB7);
        let mut d = VideoDecoder::new();
        d.set_strict(true);
        assert!(d.push(&w.bytes).is_err() || d.flush().is_err());
        let mut d = VideoDecoder::new();
        d.push(&w.bytes).unwrap();
        d.flush().unwrap();
        assert_eq!(d.stats().errors, 1);
        assert!(d.next_frame().is_some());
    }

    #[test]
    fn rgba_conversion_of_neutral_and_saturated_colors() {
        let f = Frame {
            width: 2,
            height: 2,
            y: vec![16, 235, 126, 82],
            cb: vec![128],
            cr: vec![128],
            picture_type: b'I',
            temporal_reference: 0,
            display_index: 0,
        };
        let mut out = vec![0u8; 16];
        f.to_rgba(&mut out);
        assert_eq!(&out[0..4], &[0, 0, 0, 255]);
        assert_eq!(&out[4..8], &[255, 255, 255, 255]);
        assert_eq!(out[8], out[9]);
        let red = Frame { y: vec![82; 4], cb: vec![90], cr: vec![240], ..f };
        red.to_rgba(&mut out);
        assert!(out[0] >= 250 && out[1] <= 3 && out[2] <= 3, "{:?}", &out[..4]);
    }
}
