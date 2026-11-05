//! The boundary between an effect program and the game: what an effect
//! reads each update ([`EffectWorld`]: the camera, the two unit objects and
//! their skeleton nodes) and what it hands back ([`EffectUpdate`]: GS
//! primitives, packet layers, frame copies, unit writes, the stage light,
//! the return code). Shared by the hand ports in this crate and by the
//! `roses-ee` interpreter runner (the test oracle), so both produce the same
//! values. Moved here from `roses_ee::bmodule` (30 Oct 2026), which
//! re-exports it.
/// The PAL frame's projection (distance d; y keeps the pixel aspect
/// 1074.2855 / 800; z rows as the arena's record) [emu 0x2ED7A0].
pub const ASPECT_Y: f32 = 1_074.285_5 / 800.0;
/// A texture an effect draws with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TextureKey {
    /// TEXEFF.MRG record (type 4).
    TexEff(u8),
    /// The attacker's effect RH2 in its MONSTER.MRG slot, obj+0x144 + 4 *
    /// slot (type 5).
    Own(u8),
    /// The other display buffer (TBP 0 / 0x800, PSMCT16S 512 x 512): the
    /// previous frame [F 0x209640 / 0x207FC0; emu B125].
    Frame,
    /// The offscreen render target at TBP 0x2000 holding unit `role` (0 the
    /// attacker, 1 its target) as 0x1F0FD0 last drew it there (the
    /// afterimage draws 0x1F8270 / 0x1F8280 / 0x1F82B0) [F; emu card 21].
    UnitRt(u8),
}

/// TBP0 of the offscreen target the unit-model extra draws render into.
pub const UNIT_RT_TBP: u16 = 0x2000;
/// Its TEX0: TBP 0x2000, TBW 8, PSMCT16, TW = TH = 9, TCC 1, CLD 1.
pub const UNIT_RT_TEX0: u64 = 0x2000 | (8 << 14) | (2 << 20) | (9 << 26) | (9 << 30) | (1 << 34) | (1 << 61);

impl TextureKey {
    /// The TBP0 of the texture (GS block units): the shim's own for TEXEFF
    /// (0x3400 + 0x40 k) and the attacker's RH2s (0x3800 + 0x40 e), clear of
    /// the frame buffers and render targets the game itself computes.
    pub fn tbp(self) -> u16 {
        match self {
            Self::TexEff(k) => 0x3400 + 0x40 * u16::from(k & 0xF),
            Self::Own(k) => 0x3800 + 0x40 * u16::from(k & 7),
            Self::Frame => 0,
            Self::UnitRt(_) => UNIT_RT_TBP,
        }
    }
    /// The shim's texture or a frame buffer at `tbp` (a render target needs
    /// the role, see [`EffectUpdate`]).
    pub fn from_tbp(tbp: u16) -> Option<Self> {
        match tbp {
            0 | 0x800 => Some(Self::Frame),
            0x3400..=0x37C0 if tbp & 0x3F == 0 => Some(Self::TexEff(((tbp - 0x3400) / 0x40) as u8)),
            0x3800..=0x39C0 if tbp & 0x3F == 0 => Some(Self::Own(((tbp - 0x3800) / 0x40) as u8)),
            _ => None,
        }
    }
}

/// RH2 records the module may ask for (from the user's import).
#[derive(Clone, Debug, Default)]
pub struct EffectTextures {
    /// TEXEFF.MRG records by id (type 4).
    pub texeff: Vec<Option<Vec<u8>>>,
    /// The attacker's effect RH2s by effect slot 0..8 (type 5).
    pub own: Vec<Option<Vec<u8>>>,
    /// Other archives' records a C module loads, as (type, record, RH2)
    /// (e.g. c401's TEXEVE records, type 8): placed as `TextureKey::Own(i)`
    /// for the i-th entry (at most 8, when `own` is empty).
    pub other: Vec<(i32, i32, Vec<u8>)>,
}

/// One vertex as the GS receives it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GsVertex {
    /// Window coordinates: XYZ2 x / 16 - 1792 (the 512-wide frame's
    /// XYOFFSET), y likewise.
    pub x: f32,
    pub y: f32,
    pub z: u32,
    /// RGBAQ bytes (alpha 0x80 = 1.0).
    pub rgba: [u8; 4],
    pub q: f32,
    /// UV in texels (FST) or ST (with q).
    pub uv: [f32; 2],
}

/// A primitive from a descriptor (0x206F30).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GsPrim {
    /// Descriptor flags (+2): kind bits 0-3, 0x10 IIP, 0x20 TME, 0x100 FGE,
    /// 0x200 ABE (+ ALPHA), 0x400 AA1, 0x800 FST, 0x1000 context 2, 0x2000
    /// FIX, 0x4000 CLAMP, 0x8000 Z write.
    pub flags: u16,
    /// GS PRIM type: 0 point, 1 line, 2 line strip, 3 triangle, 4 strip,
    /// 5 fan, 6 sprite.
    pub kind: u8,
    pub vertices: Vec<GsVertex>,
    pub alpha: Option<u64>,
    pub tex0: Option<u64>,
    /// TEX1 (filters, LOD K / L), with TEX0.
    pub tex1: Option<u64>,
    /// CLAMP when the descriptor writes it (flag 0x4000); else the GS keeps
    /// the previous draw's, REPEAT in every dump checked [emu 062-066: the
    /// frame-texture strips 0xA34; card 21's 0x226 composite].
    pub clamp: Option<u64>,
    /// The texture TEX0.TBP0 points at, when it is one this shim placed.
    pub texture: Option<TextureKey>,
    /// Written into a caller buffer (0x206F90) rather than the frame.
    pub buffered: bool,
    /// The descriptor's address (research).
    pub desc: u32,
    /// The ordering-table key it was linked at (0x206F30's a1 / a2: the
    /// OT 0x2E91B8, drawn from the highest key down, later links of a key
    /// first) [F 0x207970; emu OT links in footage 044 / 064].
    pub ot_key: Option<i32>,
    /// The packet layer (0x2078A0 ... 0x207910) it was written into.
    pub layer: Option<u32>,
    /// Drawn into this role's offscreen unit target (FRAME 0x2000): sent by
    /// the module's callback that 0x1EF8F0 calls between the unit's render
    /// there and its composite [F 0x1F080C..0x1F0818; emu card 484 c20
    /// draw 16, the "glass" strip].
    pub target: Option<u8>,
    /// Sent by the pre-model callback (0x1EF8F0's t3): drawn into the target
    /// before the unit's model [F 0x1EFE70].
    pub target_pre: bool,
    /// Drawn after the update's first frame copy (GS order, set by the
    /// caller from [`copy_position`]): not part of what the copy holds.
    pub post_copy: bool,
    /// TEX0 points at the buffer being drawn this frame (0x209640(1): TBP 0
    /// on odd frame counters 0x2E963C, else 0x800 [F]): the low-RAM TEX0 0
    /// on every other frame, reading what the frame holds so far.
    pub drawn_buffer: bool,
}

impl GsPrim {
    pub fn gouraud(&self) -> bool {
        self.flags & 0x10 != 0
    }
    pub fn textured(&self) -> bool {
        self.flags & 0x20 != 0
    }
    pub fn blended(&self) -> bool {
        self.flags & 0x200 != 0
    }
    pub fn uv_texels(&self) -> bool {
        self.flags & 0x800 != 0
    }
    /// TEX0 (TW, TH) as texel sizes.
    pub fn texture_size(&self) -> Option<(u32, u32)> {
        let t = self.tex0?;
        Some((1 << ((t >> 26) & 15), 1 << ((t >> 30) & 15)))
    }
}

/// The arena camera as the port draws it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraMirror {
    pub eye: [f32; 3],
    pub target: [f32; 3],
    /// Projection distance (camera +0x44, 256 / tan(fov / 2)).
    pub distance: f32,
    pub roll: f32,
}

impl CameraMirror {
    /// The view matrix (row vectors v' = v M): right, up (y down) and
    /// forward columns from eye and target, as the arena's look-at [emu
    /// 0x2ED760 in footage 044 against its eye +0x100 and dir +0x110].
    pub fn view(&self) -> [[f32; 4]; 4] {
        let f = normalize(sub(self.target, self.eye)).unwrap_or([0.0, 0.0, 1.0]);
        let r = normalize(cross(f, [0.0, 1.0, 0.0])).unwrap_or([1.0, 0.0, 0.0]);
        let u = cross(f, r);
        let t = [-dot(r, self.eye), -dot(u, self.eye), -dot(f, self.eye)];
        [
            [r[0], u[0], f[0], 0.0],
            [r[1], u[1], f[1], 0.0],
            [r[2], u[2], f[2], 0.0],
            [t[0], t[1], t[2], 1.0],
        ]
    }
    /// The projection record (0x2ED7A0).
    pub fn projection(&self) -> [[f32; 4]; 4] {
        let d = self.distance;
        [[d, 0.0, 0.0, 0.0], [0.0, d * ASPECT_Y, 0.0, 0.0], [2048.0, 2048.0, -2048.25, 1.0], [0.0, 0.0, 16_779_264.0, 0.0]]
    }
}

pub fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
pub fn normalize(a: [f32; 3]) -> Option<[f32; 3]> {
    let l = dot(a, a).sqrt();
    (l > 1e-12).then(|| [a[0] / l, a[1] / l, a[2] / l])
}
pub fn mat_mul(a: &[[f32; 4]; 4], b: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..4).map(|k| a[i][k] * b[k][j]).sum()))
}

/// One arena unit object as the module sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct UnitMirror {
    /// obj+0x00.
    pub position: [f32; 3],
    /// obj+0x10 (s32 angles, 0x10000 = a turn).
    pub rotation: [i32; 3],
    /// obj+0x80, the animated position.
    pub animated: [f32; 3],
    /// obj+0x1E0 current motion, obj+0x19C its time, obj+0x198 speed.
    pub motion: u8,
    pub time: f32,
    pub speed: f32,
    /// obj+0x1F7 top bits: 1 forward, -1 backward, 0 stopped.
    pub play_mode: i8,
    /// obj+0x20D bit 0 (0x1F84D0 -> 0x1F4500 sets it): the motion step
    /// 0x1EE6E0 and the cue player 0x1EEC80 skip the unit [F 0x1EE7B4,
    /// 0x1EED60]; B220 holds Orion at t 10 with it [emu battle-pine 036-043].
    pub frozen: bool,
    /// Frame counts by motion (obj+0x1C0 + 4 m).
    pub frames: [u32; 8],
    /// obj+0x12F (0x80 opaque).
    pub alpha: u8,
    /// obj+0x1FE, the skeleton's node count.
    pub nodes: u8,
    /// obj+0x206 (monster model index).
    pub model: u16,
    /// Node 0's bounding sphere: centre obj+0x90, radius obj+0x1E4 [emu].
    pub bounds: [f32; 4],
}

/// Entries of the ordering table 0x2E91B8 (log2 12 at +0xC) [emu].
pub const OT_ENTRIES: u32 = 4096;

/// The stage light byte (0x1F7F10 sets it, 0x1F7F40 reads it).
pub const STAGE_LIGHT: u32 = 0x002D_F5B2;

/// The GS order of an update's primitives (indices into `prims`): the ones
/// sent straight (no ordering table, outside a layer) first in submission
/// order [H]; then the ordering table 0x2E91B8 from its highest key down,
/// the later links of a key first (entry k links to k - 1 and a link is
/// pushed at the head) [F 0x206F30 / 0x207910]; a packet layer is one link
/// whose primitives keep their submission order.
pub fn draw_order(prims: &[GsPrim], layers: &[(u32, Option<i32>, usize)]) -> Vec<usize> {
    // (key, link sequence, primitives)
    let mut links: Vec<(i32, usize, Vec<usize>)> = Vec::new();
    let mut direct = Vec::new();
    let linked_layer = |id: u32| layers.iter().find(|l| l.0 == id && l.1.is_some());
    for (i, p) in prims.iter().enumerate() {
        match (p.layer.and_then(linked_layer), p.ot_key) {
            (Some(_), _) => {}
            (None, Some(k)) => links.push((k, 2 * i + 1, vec![i])),
            (None, None) => direct.push(i),
        }
    }
    for &(id, key, at) in layers {
        let Some(k) = key else { continue };
        let members: Vec<usize> = prims.iter().enumerate().filter(|(_, p)| p.layer == Some(id)).map(|(i, _)| i).collect();
        // Linked after primitive at - 1 (sequence 2 at - 1), before
        // primitive at (2 at + 1).
        links.push((k, 2 * at, members));
    }
    // Later links of a key first.
    links.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    direct.into_iter().chain(links.into_iter().flat_map(|l| l.2)).collect()
}

/// A GS local copy of the frame queued by 0x207FC0 into the packet being
/// written: its destination, the primitives submitted before it and the
/// packet layer it went into (none: sent straight).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameCopy {
    pub tbp: u16,
    pub before: usize,
    pub layer: Option<u32>,
}

/// Where a frame copy lands in the GS order of [`draw_order`]: the number of
/// primitives drawn before it. In a linked layer it keeps its submission
/// place among the layer's primitives; else it is sent straight, among the
/// straight primitives in submission order [F 0x207FC0 writes at the packet
/// pointer it is given, as 0x206F90 does].
pub fn copy_position(prims: &[GsPrim], layers: &[(u32, Option<i32>, usize)], copy: &FrameCopy) -> usize {
    // The copy as a marker primitive at its submission place.
    let at = copy.before.min(prims.len());
    let mut all: Vec<GsPrim> = Vec::with_capacity(prims.len() + 1);
    all.extend_from_slice(&prims[..at]);
    all.push(GsPrim { kind: 7, layer: copy.layer, ..Default::default() });
    all.extend_from_slice(&prims[at..]);
    // Links made after the copy move by one; its own layer closed after it.
    let shifted: Vec<(u32, Option<i32>, usize)> =
        layers.iter().map(|&(id, key, n)| (id, key, if n > at || (n == at && Some(id) == copy.layer) { n + 1 } else { n })).collect();
    draw_order(&all, &shifted).iter().position(|&i| i == at).unwrap_or(prims.len())
}

/// What the outcome boundary 0x1F8100 reads, as plain values (helper D30,
/// 3 Nov 2026; OUTCOME-BOUNDARY.md): the battle's two handles, the active
/// side and both units' stats at the defender's cell. The caller computes
/// the stats (in the game `Query::stats` = 0x267A00 + 0x1E1570 / 0x1E1650 at
/// the cell); [`BattleState::answer`] is only the 0x1F8160 / 0x1DF530 head.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BattleState {
    /// The battle's handles (side << 8 | slot): role 0 (attacker) 0x2DE298,
    /// role 1 (defender) 0x2DE29C [F 0x1E64C0].
    pub roles: [u16; 2],
    /// The duel's active side u8 0x2DB8F4 (0x1DF7C0) [F].
    pub active_side: u8,
    /// The defender's (role 1's) cell (x, z): +0x40 / +0x41 of its record,
    /// +0x42 / +0x43 while +0x57 bit 6 is set [F 0x1F820C]; where the stats
    /// are taken (the head itself does not read it).
    pub cell: (u8, u8),
    /// The units by role; `None`: no unit record (0x1E07C0 = 0: +0x45 >=
    /// 0xB or no such slot).
    pub units: [Option<BattleUnit>; 2],
}

/// One battle unit's stats at the defender's cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BattleUnit {
    /// 0x267A00(rec, cx, cz, 1) + 0x1E1570(rec, 0) (the rule clamps it to
    /// 0..9999) [F 0x1DF5A0..0x1DF5E8].
    pub attack: i32,
    /// 0x267A00(rec, cx, cz, 0) + 0x1E1650(rec, 0).
    pub defence: i32,
    /// +0x57 bit 3: in defence position [F 0x1DF6C8].
    pub defence_position: bool,
}

impl BattleState {
    /// The stats 0x1E07C0(side, slot) finds: a role's unit when the handle
    /// is a board slot (side 0..2, slot 0..10); the later role when both name
    /// the same slot (`world::mirror_battle` writes role 1 last).
    pub fn unit(&self, side: i32, slot: i32) -> Option<BattleUnit> {
        if !(0..3).contains(&side) || !(0..11).contains(&slot) {
            return None;
        }
        (0..2).rev().find(|&r| i32::from(self.roles[r] >> 8) == side && i32::from(self.roles[r] & 0xFF) == slot).and_then(|r| self.units[r])
    }

    /// 0x1DF530(sa, la, sd, ld, cx, cz, flag): 4 = a destroys d, 3 = it
    /// does not (a tie with flag 0 and a on the active side), 6 = d's
    /// defence holds, 0 = a record missing [F; ARENA-ATTACK.md 4].
    pub fn rule(&self, a: (i32, i32), d: (i32, i32), flag: bool) -> i32 {
        let (Some(ra), Some(rd)) = (self.unit(a.0, a.1), self.unit(d.0, d.1)) else { return 0 };
        let c = |v: i32| v.clamp(0, 9999);
        let (atk_a, atk_d, def_d) = (c(ra.attack), c(rd.attack), c(rd.defence));
        if rd.defence_position {
            return if atk_a > def_d { 4 } else { 6 };
        }
        if atk_a > atk_d {
            4
        } else if atk_a < atk_d {
            3
        } else if !flag && a.0 == i32::from(self.active_side) {
            3
        } else {
            4
        }
    }

    /// 0x1F8160(flag) for the module's own / target handle words
    /// (0x2DF5AC / 0x2DF5A8): -1 without a role-1 unit or with a handle off
    /// the board, else the rule for (own, target) [F].
    pub fn answer(&self, own: u32, target: u32, flag: bool) -> i32 {
        let r1 = self.roles[1];
        if self.unit(i32::from(r1 >> 8), i32::from(r1 & 0xFF)).is_none() {
            return -1;
        }
        let (os, ol, ts, tl) = ((own as i32) >> 8, (own & 0xFF) as i32, (target as i32) >> 8, (target & 0xFF) as i32);
        if !(0..3).contains(&os) || !(0..11).contains(&ol) || !(0..3).contains(&ts) || !(0..11).contains(&tl) {
            return -1;
        }
        self.rule((os, ol), (ts, tl), flag)
    }

    /// 0x1F8100(): 1 iff the module's own unit destroys its target, flag =
    /// role 0 is not the own unit (a counter effect) [F].
    pub fn outcome(&self, own: u32, target: u32) -> i32 {
        i32::from(self.answer(own, target, u32::from(self.roles[0]) != own) == 4)
    }
}

/// What the runner reads from the battle each update.
pub trait EffectWorld {
    fn camera(&self) -> CameraMirror;
    /// Role 0 attacker, 1 defender.
    fn unit(&self, role: usize) -> UnitMirror;
    /// World position of skeleton node `node` of unit `role` (obj+0xA0 +
    /// 16 * slot as the model renderer fills it).
    fn node(&self, role: usize, node: u8) -> Option<[f32; 3]>;
    /// The battle the outcome boundary 0x1F8100 reads ([`BattleState`]);
    /// `None` (the default): not mirrored, the zero image's answer 1
    ///.
    fn battle(&self) -> Option<BattleState> {
        None
    }
    /// The display frame counter 0x2E963C the module sees (one count per
    /// update, shared by every module of the battle); `None`: the calls made.
    fn frame_counter(&self) -> Option<u32> {
        None
    }
}

/// One update's output.
#[derive(Clone, Debug, Default)]
pub struct EffectUpdate {
    /// The module's return (8 = end).
    pub ret: i32,
    /// Primitives in submission order.
    pub prims: Vec<GsPrim>,
    /// Boundary calls this shim does not implement: (address, name, a0..a3).
    pub boundary: Vec<(u32, &'static str, [u32; 4])>,
    /// 0x202530 asks for textures not available (type, id).
    pub missing_textures: Vec<(i32, i32)>,
    /// obj+0x20D of (attacker, defender) after the update.
    pub flags_20d: [u8; 2],
    /// The stage light 0x2DF5B2 after the update (0x80 = 1.0: the stage
    /// objects' light scale g; modules set it through 0x1F7F10) [F
    /// ARENA-ENVIRONMENT.md 3; run: modules 373 / 133 dim the stage].
    pub stage_light: u8,
    /// Units drawn into the offscreen target by 0x1F0FD0 (0x1EF6F0 /
    /// 0x1EF8F0), in order, with the number of primitives emitted before.
    pub unit_renders: Vec<UnitRender>,
    /// Unit object fields the module wrote, read back after the update.
    pub unit_writes: [UnitWrites; 2],
    /// Packet layers closed this update: (id, OT key when linked, number
    /// of primitives submitted before the link).
    pub layers: Vec<(u32, Option<i32>, usize)>,
    /// GS local copies of the frame queued (0x207FC0).
    pub frame_copies: Vec<FrameCopy>,
    pub steps: u64,
}

/// A unit drawn into the offscreen target (TBP 0x2000) for an afterimage.
#[derive(Clone, Debug, PartialEq)]
pub struct UnitRender {
    pub role: u8,
    /// Primitives emitted before it in this update.
    pub before: usize,
    /// 0x1EF8F0's t3 (module t0 of 0x1F82B0): not a matrix but a module
    /// callback run before the unit's model, into the target (0 = none)
    /// [F 0x1EF96C / 0x1EFE64..0x1EFE78].
    pub pre: u32,
    /// The camera the unit is drawn into the target with when the pre
    /// callback switched it (0x20D660 / 0x20DAD0 / 0x20E3D0): the current
    /// camera record's view-projection (0x2ED720 + 0x1A0 * index 0x2ED6C0,
    /// +0, row vectors) as the callback left it [F 0x20E3D0].
    pub pre_vp: Option<[[f32; 4]; 4]>,
    /// 0x1EF8F0's stack argument (module t1), when given.
    pub extra: u64,
}

/// Unit object fields modules write that change how the unit looks
/// [run: the sweep's object writes].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UnitWrites {
    /// obj+0x12C..0x12F: RGBA multiplier (0x80 = 1.0; alpha +0x12F).
    pub tint: [u8; 4],
    /// obj+0x130..0x133 (written with the tint by 24 modules) [H: an added colour].
    pub add: [u8; 4],
    /// obj+0x30: scale (x, y, z, w).
    pub scale: [f32; 4],
    /// obj+0x1F8: bits 0-1 pick the offscreen composite's ALPHA, bits 2-3 the
    /// full-window colour pass's (0x208450: table 0x2D0FA0 = 0x44, 0x48, 0x42,
    /// 0x84, FIX 0x80) [F 0x1EFD08 / 0x1F06EC / 0x1F0848].
    pub mode: u8,
    /// obj+0x128: the float the colour pass's alpha is scaled by (+0x133 *
    /// +0x128, 0x1A2C70) [F 0x1F0720..0x1F0760].
    pub fade: f32,
}

