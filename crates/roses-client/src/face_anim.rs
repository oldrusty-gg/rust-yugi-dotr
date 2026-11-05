//! Talking-portrait ("face animation") engine of PAL SLES_515.79: the small
//! speaker portraits of the campaign map's hover panel and the story panels.
//! Source facts (private research `story/the project notes; [F] code or
//! data, [emu] EE interpreter oracle `run_faces.py`):
//!
//! - Requests (0x247310): 8 bytes {slot (255 = allocate), speaker,
//!   expression, kind, x: i16, y: i16}. Allocation takes the first of 8 free
//!   slots at 0x3146D0 (stride 52) and marks its expression 255; whenever the
//!   requested expression differs from the slot's, all 16 timers (+11) and 16
//!   frame indices (+27) are zeroed; then speaker/expression/kind/x/y are
//!   copied (+4, +5, +6, +44, +46) [F]. 0x2474B0 frees a slot [F].
//! - Tables (no runtime state): pointer table 0x29C870 indexed by speaker
//!   (22 entries, then null) → root {rects, expressions, nodes, expression
//!   count}. rect = 4 × i16 {v0, u0, v1, u1}; expression = 8 bytes {layer
//!   count, first node}; node = 8 bytes {next (0/255 = end), flags, ...}.
//!   A static node (flags & 0x80 = 0) is {next, flags, sprite, -, dx: u16,
//!   dy: u16}; an animated node is {next, 0x80, type: u16, frame count:
//!   u16, first frame node: u8, -}; a frame node is {next, duration, sprite,
//!   -, dx: u16, dy: u16} [F/emu].
//! - Per frame (0x247A10, for every active slot with a texture): three rand
//!   calls r1..r3 (0x19AE88, newlib `rand`), s0 = (r1 >> 4) & 63,
//!   blink = s0 + ((r2 >> 4) & 63), talk = (r3 >> 4) & 7. Layers are drawn
//!   in order; the k-th animated layer owns timer[k] and frame[k]. If kind
//!   has bit `type` set: a non-zero timer counts down; a zero timer (after
//!   the count) advances the frame; frame >= count wraps to 0. Otherwise the
//!   frame is 0. A layer with count 0 is skipped. After drawing, a zero timer
//!   reloads with the frame's duration, + talk for type 1 (mouth), + blink
//!   for type 0 (eyes) on frame 0 [F, emu].
//! - Each sprite (0x2474E0): texture rect {u0, v0, u1, v1}, destination
//!   x + ⌊0.8·dx + ½⌋, y + ⌊1.125·dy + ½⌋, size ⌊0.8·(u1−u0) + ½⌋ ×
//!   ⌊1.125·(v1−v0) + ½⌋ in the 512×448 frame, colour (128,128,128,128),
//!   CLAMP region [u0,u1]×[v0,v1], ALPHA 0x44 ((Cs−Cd)·As + Cd), texture =
//!   TEXANM.MRG record `speaker` (0x202530(7, speaker)) [F, emu].
//! - Kind is a bit set over layer types: kind 1 blinks only, kind 3 blinks
//!   and talks [F]. Type 0 = eyes, 1 = mouth [F, emu: Weevil].
use crate::duel_hud::elf_slice;
use crate::Result;
use serde::Serialize;

/// Pointer table of the per-speaker face tables.
pub const FACE_ROOTS: u32 = 0x29C870;
/// Expression byte per speaker and mood: 0x298DAC + 4·speaker + mood.
pub const EXPRESSION_TABLE: u32 = 0x298DAC;
const MAX_SPEAKERS: usize = 32;
/// Timers / frame indices per slot.
pub const ANIMATED_LAYERS: usize = 16;
/// ALPHA register of every face sprite.
pub const FACE_ALPHA: u64 = 0x44;

/// One speaker's tables (root of 0x29C870[speaker]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SpeakerFace {
    /// {v0, u0, v1, u1} texel rects.
    pub rects: Vec<[u16; 4]>,
    /// {layer count, first node, ...}.
    pub expressions: Vec<[u8; 8]>,
    pub nodes: Vec<[u8; 8]>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FaceTables {
    pub speakers: Vec<SpeakerFace>,
    /// 0x298DAC: four expression bytes per speaker.
    pub moods: Vec<[u8; 4]>,
}

fn rd32(exe: &[u8], address: u32) -> Result<u32> {
    let b = elf_slice(exe, address, 4)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn records(exe: &[u8], address: u32, count: usize) -> Result<Vec<[u8; 8]>> {
    Ok(elf_slice(exe, address, count * 8)?
        .chunks_exact(8)
        .map(|c| c.try_into().unwrap())
        .collect())
}

impl FaceTables {
    /// Read every speaker's tables from the user's executable.
    pub fn from_executable(exe: &[u8]) -> Result<Self> {
        let mut speakers = Vec::new();
        for s in 0..MAX_SPEAKERS as u32 {
            let root = rd32(exe, FACE_ROOTS + 4 * s)?;
            if root == 0 {
                break;
            }
            let (rects, exprs, nodes) = (rd32(exe, root)?, rd32(exe, root + 4)?, rd32(exe, root + 8)?);
            let count = rd32(exe, root + 12)? as usize;
            // Layout of every entry: rects, expressions, nodes, then the root.
            if !(rects < exprs && exprs <= nodes && nodes < root) || count > 64 {
                return Err(format!("face table of speaker {s} at EE 0x{root:X} has an unknown layout"));
            }
            speakers.push(SpeakerFace {
                rects: records(exe, rects, ((exprs - rects) / 8) as usize)?
                    .into_iter()
                    .map(|r| std::array::from_fn(|i| u16::from_le_bytes([r[2 * i], r[2 * i + 1]])))
                    .collect(),
                expressions: records(exe, exprs, count)?,
                nodes: records(exe, nodes, ((root - nodes) / 8) as usize)?,
            });
        }
        if speakers.is_empty() {
            return Err("no face tables at EE 0x29C870".into());
        }
        let moods = elf_slice(exe, EXPRESSION_TABLE, 4 * speakers.len())?
            .chunks_exact(4)
            .map(|c| c.try_into().unwrap())
            .collect();
        Ok(Self { speakers, moods })
    }

    /// The expression the hover panel requests (0x2356A0: table byte at
    /// 0x298DAC + 4·speaker + mood, mood = signed W+546A).
    pub fn expression(&self, speaker: u8, mood: u8) -> Option<u8> {
        self.moods.get(speaker as usize)?.get(mood as usize).copied()
    }

    /// A newly allocated face (0x247310 with slot 255).
    pub fn face(&self, speaker: u8, expression: u8, kind: u8, x: i16, y: i16) -> Result<Face> {
        Face::new(self, speaker, expression, kind, x, y)
    }
}

impl SpeakerFace {
    fn node(&self, i: u8) -> Option<&[u8; 8]> {
        self.nodes.get(i as usize)
    }

    /// Follow `steps` links from node `start`, stopping at 0 / 255
    /// (0x247160 / 0x247210).
    fn walk(&self, start: u8, steps: usize) -> Option<u8> {
        let mut at = start;
        for _ in 0..steps {
            match self.node(at)?[0] {
                0 | 255 => break,
                n => at = n,
            }
        }
        Some(at)
    }

    /// Node of layer `layer` of `expression` (0x247160).
    fn layer(&self, expression: u8, layer: usize) -> Option<[u8; 8]> {
        let first = self.expressions.get(expression as usize)?[1];
        self.node(self.walk(first, layer)?).copied()
    }

    /// Frame node `frame` of an animated layer node (0x247210).
    fn frame(&self, layer: &[u8; 8], frame: u8) -> Option<[u8; 8]> {
        self.node(self.walk(layer[6], frame as usize)?).copied()
    }
}

fn h(r: &[u8; 8], i: usize) -> u16 {
    u16::from_le_bytes([r[2 * i], r[2 * i + 1]])
}

/// One sprite as 0x2474E0 emits it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct FaceDraw {
    pub layer: u8,
    pub sprite: u8,
    /// Texels {u0, v0, u1, v1}; the CLAMP region is [u0, u1] × [v0, v1].
    pub tex_rect: [u16; 4],
    /// {x0, y0, x1, y1} in the 512×448 frame.
    pub dest_rect: [i32; 4],
    pub rgba: [u8; 4],
    /// CLAMP register word (REGION_CLAMP both axes).
    pub clamp: u64,
    /// ALPHA register word.
    pub alpha: u64,
}

/// One face slot (0x3146D0 + 52·n).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Face {
    table: SpeakerFace,
    pub speaker: u8,
    pub expression: u8,
    pub kind: u8,
    pub x: i16,
    pub y: i16,
    /// +11: per animated layer countdown.
    pub timers: [u8; ANIMATED_LAYERS],
    /// +27: per animated layer frame index.
    pub frames: [u8; ANIMATED_LAYERS],
}

/// Round as the game does: float, scale, + ½, truncate to u16.
fn scaled(v: f32, k: f32) -> u32 {
    ((0.5f32 + k * v) as u32) & 0xFFFF
}

/// A GS coordinate (16-bit, 4 fractional bits, offset 1792) back to pixels.
fn gs(base: i16, add: u32) -> i32 {
    let raw = (base as u16 as u32 + 1792 + add) & 0xFFF;
    raw as i32 - 1792
}

impl Face {
    pub fn new(tables: &FaceTables, speaker: u8, expression: u8, kind: u8, x: i16, y: i16) -> Result<Self> {
        let table = tables
            .speakers
            .get(speaker as usize)
            .ok_or_else(|| format!("no face table for speaker {speaker}"))?
            .clone();
        Ok(Self {
            table,
            speaker,
            expression,
            kind,
            x,
            y,
            timers: [0; ANIMATED_LAYERS],
            frames: [0; ANIMATED_LAYERS],
        })
    }

    /// A slot exactly as the game holds it (52 bytes at 0x3146D0 + 52·n).
    pub fn from_slot(tables: &FaceTables, slot: &[u8]) -> Result<Self> {
        if slot.len() < 48 {
            return Err("face slot is 52 bytes".into());
        }
        let mut face = Self::new(
            tables,
            slot[4],
            slot[5],
            slot[6],
            i16::from_le_bytes([slot[44], slot[45]]),
            i16::from_le_bytes([slot[46], slot[47]]),
        )?;
        face.timers.copy_from_slice(&slot[11..27]);
        face.frames.copy_from_slice(&slot[27..43]);
        Ok(face)
    }

    /// Re-issue a request to this slot (0x247310 with its slot number): a
    /// different expression restarts every timer and frame.
    pub fn request(&mut self, expression: u8, kind: u8, x: i16, y: i16) {
        if expression != self.expression {
            self.timers = [0; ANIMATED_LAYERS];
            self.frames = [0; ANIMATED_LAYERS];
        }
        (self.expression, self.kind, self.x, self.y) = (expression, kind, x, y);
    }

    /// One frame of 0x247A10. `rng` is the game's `rand` (0..2³¹); it is
    /// called exactly three times.
    pub fn step(&mut self, rng: &mut impl FnMut() -> i32) {
        let s0 = (rng() >> 4) & 63;
        let blink = (s0 + ((rng() >> 4) & 63)) as u8;
        let talk = ((rng() >> 4) & 7) as u8;
        let t = &self.table;
        let Some(expr) = t.expressions.get(self.expression as usize) else { return };
        let mut k = 0usize;
        for layer in 0..expr[0] as usize {
            let Some(node) = t.layer(self.expression, layer) else { continue };
            if node[1] & 0x80 == 0 {
                continue;
            }
            if k >= ANIMATED_LAYERS {
                break;
            }
            let (kind_bit, count) = (h(&node, 1), h(&node, 2));
            if kind_bit < 8 && self.kind & (1 << kind_bit) != 0 {
                if self.timers[k] != 0 {
                    self.timers[k] -= 1;
                }
                if self.timers[k] == 0 {
                    self.frames[k] = self.frames[k].wrapping_add(1);
                }
                if u16::from(self.frames[k]) >= count {
                    self.frames[k] = 0;
                }
            } else {
                self.frames[k] = 0;
            }
            if count != 0 && self.timers[k] == 0 {
                if let Some(frame) = t.frame(&node, self.frames[k]) {
                    let duration = frame[1];
                    self.timers[k] = match kind_bit {
                        1 => duration.wrapping_add(talk),
                        0 if self.frames[k] == 0 => duration.wrapping_add(blink),
                        _ => duration,
                    };
                }
            }
            k += 1;
        }
    }

    /// The sprites 0x247A10 emits for the current state (what the game
    /// draws in the frame of the latest [`Face::step`]).
    pub fn draws(&self) -> Vec<FaceDraw> {
        let t = &self.table;
        let mut out = Vec::new();
        let Some(expr) = t.expressions.get(self.expression as usize) else { return out };
        let mut k = 0usize;
        for layer in 0..expr[0] as usize {
            let Some(node) = t.layer(self.expression, layer) else { continue };
            let (sprite, dx, dy) = if node[1] & 0x80 == 0 {
                (node[2], h(&node, 2), h(&node, 3))
            } else {
                let count = h(&node, 2);
                let frame = self.frames.get(k).copied().unwrap_or(0);
                k += 1;
                if count == 0 {
                    continue;
                }
                let Some(f) = t.frame(&node, frame) else { continue };
                (f[2], h(&f, 2), h(&f, 3))
            };
            let Some(&[v0, u0, v1, u1]) = t.rects.get(sprite as usize) else { continue };
            let (ox, oy) = (scaled(dx as f32, 0.8), scaled(dy as f32, 1.125));
            let w = scaled(u1 as f32 - u0 as f32, 0.8);
            let hgt = scaled(v1 as f32 - v0 as f32, 1.125);
            let (x0, y0) = (gs(self.x, ox), gs(self.y, oy));
            let (x1, y1) = (gs(self.x, (ox + w) & 0xFFFF), gs(self.y, (oy + hgt) & 0xFFFF));
            let field = |v: u16| u64::from(v) & 0x3FF;
            out.push(FaceDraw {
                layer: layer as u8,
                sprite,
                tex_rect: [u0, v0, u1, v1],
                dest_rect: [x0, y0, x1, y1],
                rgba: [128; 4],
                clamp: 0xA | field(u0) << 4 | field(u1) << 14 | field(v0) << 24 | field(v1) << 34,
                alpha: FACE_ALPHA,
            });
        }
        out
    }
}

/// newlib `rand` as 0x19AE88 runs it: state·0x5851F42D4C957F2D + 1,
/// result = bits 32..62. The state lives at *(0x28ED18) + 0xA8.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NewlibRand(pub u64);

impl NewlibRand {
    pub fn next(&mut self) -> i32 {
        self.0 = self.0.wrapping_mul(0x5851_F42D_4C95_7F2D).wrapping_add(1);
        ((self.0 >> 32) & 0x7FFF_FFFF) as i32
    }
}


/// Story portrait assets → face-table speaker = TEXANM record = name id −
/// 196 (Simon 197 → 1, Seto 2, Pegasus 10, Richard 11, Mako 15, Yugi 20,
/// Manawyddan 21) [emu: the panels' 0x247310 requests, research
/// `story/the project notes]. Each asset is the whole 256×256 record.
pub const STORY_PORTRAITS: [(&str, u8); 7] = [
    ("simon-small-portrait", 1),
    ("seto-small-portrait", 2),
    ("pegasus-small-portrait", 10),
    ("richard-small-portrait", 11),
    ("mako-small-portrait", 15),
    ("yugi-small-portrait", 20),
    ("manawyddan-small-portrait", 21),
];
/// Raw face origins of the story panels: bottom 0x23341C (16, 368), top
/// 0x2330B4 (400, 0) [F, emu trace-win2].
pub const PANEL_FACE_ORIGINS: [[i16; 2]; 2] = [[16, 368], [400, 0]];
/// Request kinds of the panels: 1 blinks, 3 blinks and talks.
pub const KIND_BLINK: u8 = 1;
pub const KIND_TALK: u8 = 3;

fn canvas_edge(raw: i32, num: i32, den: i32) -> u32 {
    ((raw.clamp(0, 512) * num + den / 2) / den) as u32
}

/// The sprites of `draws` as story layers of `asset` (the speaker's whole
/// TEXANM record): raw 512×512 → canvas 640×448 (x·5/4, y·7/8, rounded,
/// clamped to the raw frame), the texel rect as the source.
pub fn draw_layers(draws: &[FaceDraw], asset: &str) -> Vec<crate::Layer> {
    draws
        .iter()
        .filter_map(|d| {
            let [u0, v0, u1, v1] = d.tex_rect.map(u32::from);
            let [x0, y0, x1, y1] = d.dest_rect;
            let (cx0, cx1) = (canvas_edge(x0, 5, 4), canvas_edge(x1, 5, 4));
            let (cy0, cy1) = (canvas_edge(y0, 7, 8), canvas_edge(y1, 7, 8));
            (u1 > u0 && v1 > v0 && cx1 > cx0 && cy1 > cy0).then(|| crate::Layer {
                asset: asset.into(),
                source: Some([u0, v0, u1 - u0, v1 - v0]),
                destination: [cx0, cy0, cx1 - cx0, cy1 - cy0],
                tint: [255; 3],
                opacity: 255,
                flip_x: false,
                filter: crate::Sampling::Linear,
            })
        })
        .collect()
}

/// One panel's face in a frame's layers: the portrait-sprite layers of one
/// speaker on one side (0 bottom, 1 top).
#[derive(Clone, Debug, PartialEq, Eq)]
struct FaceGroup {
    side: usize,
    asset: String,
    speaker: u8,
    /// Indices of the group's layers.
    at: Vec<usize>,
}

fn face_groups(layers: &[crate::Layer]) -> Vec<FaceGroup> {
    let mut groups: Vec<FaceGroup> = Vec::new();
    for (i, l) in layers.iter().enumerate() {
        let Some(&(asset, speaker)) = STORY_PORTRAITS.iter().find(|(a, _)| *a == l.asset) else {
            continue;
        };
        // Bottom panel faces lie below the canvas middle, top ones above.
        let side = usize::from(l.destination[1] < 224);
        match groups.iter_mut().find(|g| g.side == side && g.asset == asset) {
            Some(g) => g.at.push(i),
            None => groups.push(FaceGroup {
                side,
                asset: asset.into(),
                speaker,
                at: vec![i],
            }),
        }
    }
    groups
}

/// The talking faces of the story panels at runtime: 0x2330B4 (top) and
/// 0x23341C (bottom) request {speaker, expression, kind, x, y} every update
/// and 0x247A10 steps every active slot once per update [F, emu].
///
/// The imported story layers carry each face at frame 0 of every animated
/// layer (`roses-cli story_faces`); the speaker comes from the asset and
/// the expression is recognised by matching those frame-0 sprites against
/// every expression of the speaker (no re-import needed). The request kind
/// is 3 (talk) on the speaking side from the update after its first reveal
/// count through its wait, else 1 (blink) (`StoryMotion::talking`) [emu
/// trace-win2 f105..f123, f278..f284]. [H]: the rand stream (the game's
/// shared newlib `rand` state at story entry is not reproduced; the slots
/// step bottom then top).
#[derive(Clone, Debug)]
pub struct StoryFaces {
    tables: FaceTables,
    slots: [Option<(String, Face)>; 2],
    rand: NewlibRand,
}

impl StoryFaces {
    pub fn new(tables: FaceTables) -> Self {
        Self {
            tables,
            slots: [None, None],
            rand: NewlibRand(1),
        }
    }

    /// The expression whose frame-0 sprites at the panel origin are the
    /// group's layers (or their last full face).
    fn expression(&self, group: &FaceGroup, layers: &[crate::Layer]) -> Option<u8> {
        let speaker = self.tables.speakers.get(usize::from(group.speaker))?;
        let [x, y] = PANEL_FACE_ORIGINS[group.side];
        // (Some imported panels repeat a sprite; repeats are ignored.)
        fn distinct<T: PartialEq>(v: impl Iterator<Item = T>) -> Vec<T> {
            let mut out = Vec::new();
            for x in v {
                if !out.contains(&x) {
                    out.push(x);
                }
            }
            out
        }
        let wants: Vec<_> = (0..speaker.expressions.len())
            .filter_map(|e| {
                let face = self.tables.face(group.speaker, e as u8, KIND_BLINK, x, y).ok()?;
                let want = distinct(draw_layers(&face.draws(), &group.asset).into_iter().map(|l| (l.source, l.destination)));
                Some((e as u8, want))
            })
            .collect();
        // The face drawn last wins: some imported sets keep an earlier
        // mood's sprites under the page's own (11160's Seto: expression 0,
        // then expression 2's layers over the shared base): the expression
        // whose base is in the group and whose other sprites end it (the
        // longest such).
        let have = distinct(group.at.iter().map(|&i| (layers[i].source, layers[i].destination)));
        wants
            .iter()
            .filter(|(_, want)| {
                !want.is_empty() && have.contains(&want[0]) && have.ends_with(&want[1..]) && have.len() >= want.len()
            })
            .max_by_key(|(_, want)| want.len())
            .map(|(e, _)| *e)
    }

    /// One update: the panels request their faces from the frame's settled
    /// layers (`talking` per side, bottom then top, gives the kind), then
    /// every face steps. A side without a face frees its slot. Returns
    /// whether the drawn sprites changed.
    pub fn step(&mut self, layers: &[crate::Layer], talking: [bool; 2]) -> bool {
        let before = self.draws();
        let groups = face_groups(layers);
        for side in 0..2 {
            let found = groups
                .iter()
                .find(|g| g.side == side)
                .and_then(|g| Some((g, self.expression(g, layers)?)));
            let Some((group, expression)) = found else {
                self.slots[side] = None;
                continue;
            };
            let kind = if talking[side] { KIND_TALK } else { KIND_BLINK };
            let [x, y] = PANEL_FACE_ORIGINS[side];
            match &mut self.slots[side] {
                Some((asset, face)) if face.speaker == group.speaker && *asset == group.asset => {
                    face.request(expression, kind, x, y)
                }
                slot => {
                    *slot = self
                        .tables
                        .face(group.speaker, expression, kind, x, y)
                        .ok()
                        .map(|f| (group.asset.clone(), f))
                }
            }
        }
        let rand = &mut self.rand;
        for (_, face) in self.slots.iter_mut().flatten() {
            face.step(&mut || rand.next());
        }
        self.draws() != before
    }

    fn draws(&self) -> [Option<Vec<FaceDraw>>; 2] {
        std::array::from_fn(|i| self.slots[i].as_ref().map(|(_, f)| f.draws()))
    }

    /// `layers` with each panel's frame-0 face replaced by the face's
    /// current sprites (a face whose slot does not hold that speaker and
    /// expression is left as imported).
    pub fn apply(&self, layers: &[crate::Layer]) -> Vec<crate::Layer> {
        let groups = face_groups(layers);
        let mut replace: Vec<(Vec<crate::Layer>, &[usize])> = Vec::new();
        for g in &groups {
            let Some((asset, face)) = &self.slots[g.side] else { continue };
            if *asset != g.asset || self.expression(g, layers) != Some(face.expression) {
                continue;
            }
            replace.push((draw_layers(&face.draws(), asset), &g.at));
        }
        if replace.is_empty() {
            return layers.to_vec();
        }
        let mut out = Vec::with_capacity(layers.len() + 4);
        for (i, l) in layers.iter().enumerate() {
            match replace.iter().find(|(_, at)| at.contains(&i)) {
                Some((sprites, at)) if at[0] == i => out.extend(sprites.iter().cloned()),
                Some(_) => {}
                None => out.push(l.clone()),
            }
        }
        out
    }

    /// The face slots (bottom, top): speaker, expression, kind.
    pub fn slots(&self) -> [Option<(u8, u8, u8)>; 2] {
        std::array::from_fn(|i| self.slots[i].as_ref().map(|(_, f)| (f.speaker, f.expression, f.kind)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic tables shaped like a portrait: base, 3-frame eyes, 3-frame
    /// mouth (no game bytes).
    fn tables() -> FaceTables {
        let n = |b: [u8; 2], c: u16, d: u16, e: u16| {
            let mut r = [0u8; 8];
            r[..2].copy_from_slice(&b);
            r[2..4].copy_from_slice(&c.to_le_bytes());
            r[4..6].copy_from_slice(&d.to_le_bytes());
            r[6..8].copy_from_slice(&e.to_le_bytes());
            r
        };
        let nodes = vec![
            [0; 8],
            n([2, 1], 0, 0, 0),         // 1 static base, sprite 0
            n([6, 0x80], 0, 3, 3),      // 2 eyes (type 0), 3 frames from 3
            n([4, 2], 1, 10, 20),       // 3
            n([5, 2], 2, 10, 20),       // 4
            n([255, 2], 3, 10, 20),     // 5
            n([255, 0x80], 1, 3, 7),    // 6 mouth (type 1), 3 frames from 7
            n([8, 1], 4, 20, 40),       // 7
            n([9, 1], 4 | 5 << 8, 20, 40), // 8 (sprite byte 4; byte 3 ignored)
            n([255, 1], 6, 20, 40),     // 9
        ];
        let rects = vec![[0, 0, 128, 128], [0, 0, 8, 80], [8, 0, 16, 80], [16, 0, 24, 80], [24, 0, 40, 32], [40, 0, 56, 32], [56, 0, 72, 32]];
        FaceTables {
            speakers: vec![SpeakerFace { rects, expressions: vec![[3, 1, 0, 0, 0, 0, 0, 0]], nodes }],
            moods: vec![[0, 0, 0, 0]],
        }
    }

    #[test]
    fn first_step_advances_and_reloads() {
        let t = tables();
        let mut f = t.face(0, 0, 1, 400, 0).unwrap();
        let mut r = [0x10i32, 0x20, 0x70].into_iter().cycle();
        f.step(&mut || r.next().unwrap());
        // Eyes: timer 0 → frame 1, duration 2 (not frame 0: no blink pause).
        assert_eq!((f.frames[0], f.timers[0]), (1, 2));
        // Mouth not in kind 1: frame 0, timer reloads with duration only... + talk.
        assert_eq!((f.frames[1], f.timers[1]), (0, 1 + 7));
        let d = f.draws();
        assert_eq!(d.len(), 3);
        assert_eq!(d[0].dest_rect, [400, 0, 502, 144]);
        assert_eq!(d[0].clamp, 0xA | 128 << 14 | 128u64 << 34);
        // Eyes frame 1 = sprite 2 at (10, 20): x + 8, y + 23 (22.5 + .5).
        assert_eq!(d[1].sprite, 2);
        assert_eq!(d[1].dest_rect, [408, 23, 408 + 64, 23 + 9]);
        assert_eq!(d[1].tex_rect, [0, 8, 80, 16]);
        assert_eq!(d[0].rgba, [128; 4]);
    }

    #[test]
    fn blink_pause_and_wrap() {
        let t = tables();
        let mut f = t.face(0, 0, 1, 0, 0).unwrap();
        // r >> 4 & 63 = 5 and 6: blink pause 11.
        let mut r = [5 << 4, 6 << 4, 0].into_iter().cycle();
        let mut seen = Vec::new();
        for _ in 0..20 {
            f.step(&mut || r.next().unwrap());
            seen.push((f.frames[0], f.timers[0]));
        }
        assert_eq!(&seen[..6], &[(1, 2), (1, 1), (2, 2), (2, 1), (0, 13), (0, 12)]);
    }

    #[test]
    fn talking_kind_and_request_reset() {
        let t = tables();
        let mut f = t.face(0, 0, 3, 0, -240).unwrap();
        let mut r = [0, 0, 3 << 4].into_iter().cycle();
        f.step(&mut || r.next().unwrap());
        assert_eq!((f.frames[1], f.timers[1]), (1, 4));
        assert_eq!(f.draws()[2].dest_rect[1], -240 + 45);
        f.request(0, 3, 0, 0);
        assert_eq!(f.frames[1], 1);
        f.request(1, 3, 0, 0);
        assert_eq!((f.frames, f.timers), ([0; 16], [0; 16]));
        assert!(f.draws().is_empty());
    }

    #[test]
    fn story_faces_recognise_animate_and_replace_the_imported_sprites() {
        let mut t = tables();
        // Speaker 1 (Simon's asset) = the synthetic face.
        t.speakers.insert(0, t.speakers[0].clone());
        let face = t.face(1, 0, KIND_BLINK, 16, 368).unwrap();
        let imported = draw_layers(&face.draws(), "simon-small-portrait");
        let mut layers = vec![crate::Layer {
            asset: "story-black".into(),
            source: None,
            destination: [0, 322, 640, 126],
            tint: [255; 3],
            opacity: 255,
            flip_x: false,
            filter: crate::Sampling::Linear,
        }];
        layers.extend(imported.iter().cloned());
        let mut faces = StoryFaces::new(t);
        // Before a step nothing is replaced.
        assert_eq!(faces.apply(&layers), layers);
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..200 {
            faces.step(&layers, [true, false]);
            assert_eq!(faces.slots()[0], Some((1, 0, KIND_TALK)));
            let out = faces.apply(&layers);
            assert_eq!(out[0], layers[0]);
            seen.insert(out[1..].iter().map(|l| l.source).collect::<Vec<_>>());
        }
        // Eyes and mouth move: more than one sprite set was drawn.
        assert!(seen.len() > 2, "{seen:?}");
        // Without the face on the page the slot is freed.
        faces.step(&layers[..1], [false; 2]);
        assert_eq!(faces.slots(), [None, None]);
    }

    /// The real face tables over the user's story import
    /// (ROSES_FACE_EXE, ROSES_STORY_MOTION_IMPORT): every portrait group of
    /// every story page is recognised, at the expression the panels request
    /// in the traces (11160 2864 Seto 2, 2872 1; 12132 3332 Seto 2; 12150
    /// Manawyddan 2; 11140 Richard/Pegasus 0) [emu], and a face before its
    /// first animated step draws exactly the imported sprites.
    #[test]
    fn private_story_faces_recognise_every_imported_panel() {
        let (Some(exe), Some(import)) = (std::env::var_os("ROSES_FACE_EXE"), std::env::var_os("ROSES_STORY_MOTION_IMPORT")) else {
            eprintln!("skipped: ROSES_FACE_EXE / ROSES_STORY_MOTION_IMPORT not set");
            return;
        };
        let tables = FaceTables::from_executable(&std::fs::read(exe).unwrap()).unwrap();
        let loaded = crate::Loaded::open(std::path::Path::new(&import)).unwrap();
        let mut model = crate::Model::new(loaded.manifest.clone()).unwrap();
        let mut checked = 0;
        let mut expect: std::collections::BTreeMap<(&str, usize), (u8, u8)> = Default::default();
        expect.insert(("map-story-11160", 0), (2, 2));
        expect.insert(("map-story-11160", 2), (2, 1));
        expect.insert(("map-story-11140", 0), (11, 0));
        for stage in loaded.manifest.stages.clone() {
            let crate::Stage::Story { id, pages, .. } = &stage else { continue };
            model.jump_to(id).unwrap();
            for (i, page) in pages.iter().enumerate() {
                let layers = loaded.story_page_layers(&model).unwrap();
                let mut faces = StoryFaces::new(tables.clone());
                for g in face_groups(&layers) {
                    let e = faces.expression(&g, &layers);
                    assert!(e.is_some(), "{id} page {i} ({}): {} not recognised", page.id, g.asset);
                    checked += 1;
                }
                if let Some(&(speaker, expression)) = expect.get(&(id.as_str(), i)) {
                    faces.step(&layers, [false; 2]);
                    let (s, e, _) = faces.slots()[0].unwrap();
                    assert_eq!((s, e), (speaker, expression), "{id} page {i}");
                }
                if i + 1 < pages.len() {
                    let _ = model.input(crate::Button::Cross);
                    if model.page_index() != i + 1 {
                        break;
                    }
                }
            }
        }
        eprintln!("{checked} portrait groups recognised");
        assert!(checked > 100);
    }

    #[test]
    fn newlib_rand_sequence() {
        let mut r = NewlibRand(1);
        let a = r.next();
        assert_eq!(a, ((0x5851_F42D_4C95_7F2Eu64 >> 32) & 0x7FFF_FFFF) as i32);
    }

    /// Compare against the interpreter log of `run_faces.py`
    /// (ROSES_FACE_EXE = SLES_515.79, ROSES_FACE_ORACLE = log JSON).
    #[test]
    #[ignore = "Requires the private executable and face oracle (ROSES_FACE_EXE, ROSES_FACE_ORACLE)"]
    fn matches_oracle() {
        let exe = std::fs::read(std::env::var_os("ROSES_FACE_EXE").expect("ROSES_FACE_EXE")).unwrap();
        let log: serde_json::Value = serde_json::from_slice(
            &std::fs::read(std::env::var_os("ROSES_FACE_ORACLE").expect("ROSES_FACE_ORACLE")).unwrap(),
        )
        .unwrap();
        let tables = FaceTables::from_executable(&exe).unwrap();
        let hex = |v: &serde_json::Value| -> Vec<u8> {
            let s = v.as_str().unwrap();
            (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
        };
        let slot0 = hex(&log["initial_slots"][0]);
        let mut face = Face::from_slot(&tables, &slot0).unwrap();
        let (mut drawn, mut rand_ok) = (0, 0);
        for f in log["frames"].as_array().unwrap() {
            for q in f["requests"].as_array().unwrap() {
                let q: Vec<i64> = q.as_array().unwrap().iter().map(|v| v.as_i64().unwrap()).collect();
                assert_eq!(q[1] as u8, face.speaker);
                face.request(q[2] as u8, q[3] as u8, q[4] as i16, q[5] as i16);
            }
            let updates = f["updates"].as_array().unwrap();
            assert_eq!(updates.len(), 1, "one face slot in frame {}", f["frame"]);
            let rands: Vec<i32> = updates[0]["rand"].as_array().unwrap().iter().map(|v| v.as_i64().unwrap() as i32).collect();
            let mut r = NewlibRand(updates[0]["rand_state_before"].as_u64().unwrap());
            assert_eq!((0..3).map(|_| r.next()).collect::<Vec<_>>(), rands, "newlib rand");
            rand_ok += 1;
            let mut it = rands.into_iter();
            face.step(&mut || it.next().unwrap());
            let want = f["draws"].as_array().unwrap();
            let got = face.draws();
            assert_eq!(got.len(), want.len(), "draw count in frame {}", f["frame"]);
            for (g, w) in got.iter().zip(want) {
                let v = w["verts"].as_array().unwrap();
                let num = |x: &serde_json::Value| x.as_f64().unwrap() as i32;
                assert_eq!(
                    g.dest_rect,
                    [num(&v[0]["x"]), num(&v[0]["y"]), num(&v[3]["x"]), num(&v[3]["y"])],
                    "frame {}",
                    f["frame"]
                );
                assert_eq!(
                    g.tex_rect.map(i32::from),
                    [num(&v[0]["u"]), num(&v[0]["v"]), num(&v[3]["u"]), num(&v[3]["v"])]
                );
                assert_eq!(format!("0x{:x}", g.clamp), w["clamp"].as_str().unwrap());
                assert_eq!(format!("0x{:x}", g.alpha), w["alpha"].as_str().unwrap());
                assert_eq!(g.rgba.to_vec(), w["rgbaq"].as_array().unwrap()[..4].iter().map(|x| x.as_u64().unwrap() as u8).collect::<Vec<_>>());
                drawn += 1;
            }
            let after = hex(&f["slots"][0]);
            assert_eq!((&after[11..27], &after[27..43]), (&face.timers[..], &face.frames[..]), "frame {}", f["frame"]);
        }
        eprintln!("{drawn} draws matched; newlib rand matched {rand_ok} frames");
        assert!(drawn > 0);
    }
}
