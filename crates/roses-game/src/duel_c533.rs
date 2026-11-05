//! CMODULE c533 (0x215), Solomon's Lawbook (event 0x21, handler 0x260760),
//! as specified in the project notes section 13 [F code;
//! emu: the reference model `tools/mf_c533_model.py` equals the
//! interpreter's primitives to 0.0625 px in five runs, light and rand counts
//! included]. Screen-anchored: both nodes sit at view (0, 0) on the
//! depth-800 plane (screen centre) every update, so target, param and camera
//! do not matter; everything Z max, OT key 0, additive (ALPHA 0x48): drawn
//! over the HUD (`TOP_LAYER`).
//! - SEAL (TEXEFF 15 (128..192, 128..192), red, half 30): alpha +4/U to 64
//!   (u 17), -8/U from u 82;
//! - BEAM (TEXEFF 4, the upper half of a ringed seal, mirrored, half 140):
//!   alpha +8/U to 128, -8/U from u 82; see `BEAM_HW`;
//! - TEXT (TEXEFF 3): from u 18 a 6 x 10 page of golden runes writes itself
//!   one glyph per update, each fading in over 4 U; fades -16/U from u 83;
//! - from u 77 (page full): the cyan library set (RING A 160 / 155, RING B
//!   160 / 170, GLOW half 130, 64 RAYS), node scale 0.1 .. 1.1 then +0.005,
//!   decays from u 82 (rings / rays 8, glow 10).
//! It also drives the BOARD LIGHT (0x1F7F00 every update): 128 - 8 (u - 1)
//! to 32 at u 13, 32 to u 81, + 8/U from u 82, 128 at u 93 ([`C533::board_light`]).
//!
//! No ret 2: returns 8 at u 93 (the handler applies the extra summon at
//! u 94 = E+94). SE bank 1: 0x29 at u 1, 0x2A at u 20, 30, .., 70, 0x2B at
//! u 77. 312 random values at init, all after srand(i): fully deterministic
//! (srand(i) per tile then 2 draws; the rays continue from srand(59) + 2 with
//! 64 x 3 draws); the game's global rand is left at that state
//! ([`game_rng_state_after_init`]).
//!
//! [H]: the BEAM width (13.5): the descriptor's aspect fields are read as
//! floats and hold a denormal; the EE interpreter keeps it (zero-width beam),
//! the R5900 FPU flushes denormals to zero (factors 1.0: the 280 x 280-unit
//! emblem). The port draws the hardware flush-to-zero result (`BEAM_HW`);
//! not checked on a GS dump.
use crate::duel_c621::FxTri;
use crate::duel_fxlib::{quad, rotm64, uvn, view_proj, Rng, SHEET_128X64, SHEET_256};

/// Module life (0x5D): returns 8 at u 93.
pub const UPDATES: u32 = 93;
/// The handler's rule change (one more summon) at u 94 (E+94).
pub const APPLY_U: u32 = 94;
/// R5900 flush-to-zero: the 280-unit emblem (13.5) [H on hardware].
pub const BEAM_HW: bool = true;
/// Drawn over everything (Z 0xFFFFFF, key 0), over the HUD.
pub const TOP_LAYER: bool = true;
/// SE (SECTL bank 1): (u, id).
pub const SE_BANK: u8 = 1;
pub const SE: [(u32, u16); 8] = [(1, 0x29), (20, 0x2A), (30, 0x2A), (40, 0x2A), (50, 0x2A), (60, 0x2A), (70, 0x2A), (77, 0x2B)];
/// Random values drawn at init.
pub const INIT_RAND: u32 = 312;
const XT: [f32; 10] = [-5.1, -4.1, -3.1, -2.1, -1.1, -0.1, 0.9, 1.9, 2.9, 3.9];
const YT: [f32; 6] = [4.1, 2.6, 1.1, -0.6, -2.1, -3.6];
/// The tiles' frames as the srand(i) draws give them (rows of 10).
pub const FRAMES: [u8; 60] = [
    0, 5, 2, 7, 5, 2, 7, 5, 2, 7, 4, 2, 7, 4, 2, 7, 4, 2, 7, 4, 1, 7, 4, 1, 7, 4, 1, 7, 4, 1, 6, 4, 1, 6, 4, 1, 6, 4, 1, 6, 3, 1, 6, 3, 1, 6, 3, 1, 6, 3, 0, 6, 3, 0, 6, 3, 0, 6, 3,
    0,
];
const CYAN: [u8; 3] = [128, 224, 224];
const TILE: f32 = 4.8;
const RAY: [[f64; 3]; 4] = [[-20.0, 400.0, 0.0], [20.0, 400.0, 0.0], [-10.0, 0.0, 0.0], [10.0, 0.0, 0.0]];

#[derive(Clone, Copy, Debug, PartialEq)]
struct Tile {
    x: f32,
    y: f32,
    f: u8,
    a: u8,
}

/// The tiles and rays of the init, and the rand state it leaves.
fn init() -> ([Tile; 60], [[i32; 3]; 64], Rng) {
    let mut r = Rng::new(0);
    let tiles = std::array::from_fn(|i| {
        let (row, col) = (i / 10, i % 10);
        r.srand(i as u32);
        let f = (r.next() & 7) as u8;
        let _ = r.next() % 5;
        Tile { x: TILE * XT[col], y: TILE * YT[row], f, a: 0 }
    });
    let rays = std::array::from_fn(|_| std::array::from_fn(|_| (r.next() & 0xFFFF) as i32));
    (tiles, rays, r)
}

/// The game's global rand state after c533's init (srand(59), then 2 + 192
/// draws): every later rand of the game continues from here [F].
pub fn game_rng_state_after_init() -> u64 {
    init().2.state()
}

#[derive(Clone, Debug, PartialEq)]
pub struct C533 {
    light: u8,
    drawn_light: u8,
    seal: u8,
    beam: u8,
    text_a: u8,
    tiles: [Tile; 60],
    count: u8,
    all: bool,
    bit0: bool,
    bit1: bool,
    s: f32,
    ring: [u8; 2],
    glow: u8,
    ray_a: u8,
    rays: [[i32; 3]; 64],
    beam_hw: bool,
    life: i32,
    done: bool,
    rand_calls: u32,
    drawn: Drawn,
    /// Additive (ALPHA 0x48) screen triangles of the last update, in draw
    /// order (TEXT, RING A, RING B, GLOW, RAYS, SEAL, BEAM), Z max, over the
    /// HUD.
    pub additive: Vec<FxTri>,
    /// Kept empty (the convention of the other modules).
    pub subtractive: Vec<FxTri>,
    pub opaque: Vec<FxTri>,
    pub alpha_blend: Vec<FxTri>,
}

/// What the last update drew.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Drawn {
    /// Primitives (quads) sent.
    pub prims: usize,
    pub seal: u8,
    pub beam: u8,
    /// TEXT descriptor alpha (used once the page is full).
    pub text: u8,
    /// Per-tile alpha of tiles 0 and 8.
    pub tile0: u8,
    pub tile8: u8,
    /// Library: node scale, ring A alpha, glow alpha (None before u 77).
    pub lib: Option<(f32, u8, u8)>,
}

impl Default for C533 {
    fn default() -> Self {
        Self::new()
    }
}

impl C533 {
    /// Init with the hardware BEAM (`BEAM_HW`).
    pub fn new() -> Self {
        Self::with_beam(BEAM_HW)
    }

    /// `beam_hw` false = the interpreter's zero-width BEAM (13.5).
    pub fn with_beam(beam_hw: bool) -> Self {
        let (tiles, rays, r) = init();
        Self {
            light: 0x80,
            drawn_light: 0x80,
            seal: 0,
            beam: 0,
            text_a: 128,
            tiles,
            count: 0,
            all: false,
            bit0: false,
            bit1: false,
            s: 0.1,
            ring: [32, 32],
            glow: 96,
            ray_a: 32,
            rays,
            beam_hw,
            life: UPDATES as i32,
            done: false,
            rand_calls: r.calls,
            drawn: Drawn::default(),
            additive: Vec::new(),
            subtractive: Vec::new(),
            opaque: Vec::new(),
            alpha_blend: Vec::new(),
        }
    }

    pub fn done(&self) -> bool {
        self.done
    }

    /// Random values drawn at init (312, after its own srand calls).
    pub fn rand_calls(&self) -> u32 {
        self.rand_calls
    }

    /// The board light (0x2DF5B1) written by the last update: write it into
    /// the board's light every update while c533 runs.
    pub fn board_light(&self) -> u8 {
        self.drawn_light
    }

    pub fn drawn(&self) -> Drawn {
        self.drawn
    }

    /// The tiles' frames (rows of 10).
    pub fn frames(&self) -> [u8; 60] {
        self.tiles.map(|t| t.f)
    }

    fn q(&mut self, anchor_s: f64, l: [[f64; 3]; 4], uv: [[f32; 2]; 4], rgba: [u8; 4], sheet: u8) {
        let mut p = [[0.0f32; 2]; 4];
        for i in 0..4 {
            match view_proj([0.0, 0.0], anchor_s, l[i]) {
                Some(v) => p[i] = v,
                None => return,
            }
        }
        quad(&mut self.additive, p, uv, [rgba; 4], true, sheet);
        self.drawn.prims += 1;
    }

    /// 0x1E00E10 type 1: 8 rim points of half `h` and the centre, 4 quads.
    fn shape1(&mut self, h: f64, s: f64, rgba: [u8; 4], uv0: [i32; 2], wh: i32) {
        let p: [[f64; 3]; 8] = [(-1.0, 0.0), (-1.0, 1.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (1.0, -1.0), (0.0, -1.0), (-1.0, -1.0)].map(|(x, y)| [h * x, h * y, 0.0]);
        let c = [0.0; 3];
        let [u0, v0] = uv0;
        let uv = uvn([[u0, v0], [u0 + wh, v0], [u0, v0 + wh], [u0 + wh, v0 + wh]], SHEET_256);
        for q in [[p[1], p[2], p[0], c], [p[3], p[2], p[4], c], [p[5], p[6], p[4], c], [p[7], p[6], p[0], c]] {
            self.q(s, q, uv, rgba, 15);
        }
    }

    /// Module update `u` (1, 2, ..). Returns 0, then 8 at u 93 (and every
    /// later call).
    pub fn update(&mut self, _u: u32) -> u32 {
        self.additive.clear();
        if self.done {
            return 8;
        }
        self.drawn = Drawn::default();
        if self.bit0 {
            // TEXT (node D, x 5, z +10).
            self.drawn.text = self.text_a;
            self.drawn.tile0 = self.tiles[0].a;
            self.drawn.tile8 = self.tiles[8].a;
            for i in 0..60 {
                let t = self.tiles[i];
                let a = if self.all { self.text_a } else { t.a };
                let (u0, v0) = (16 * i32::from(t.f % 4), 32 + 16 * i32::from(t.f / 4));
                let (x, y) = (t.x, t.y);
                let l = [[x * 5.0, y * 5.0, 10.0], [5.0 * (x + TILE), y * 5.0, 10.0], [x * 5.0, 5.0 * (y - TILE), 10.0], [5.0 * (x + TILE), 5.0 * (y - TILE), 10.0]].map(|v: [f32; 3]| v.map(f64::from));
                self.q(1.0, l, uvn([[u0, v0], [u0 + 15, v0], [u0, v0 + 15], [u0 + 15, v0 + 15]], SHEET_128X64), [128, 128, 128, a], 3);
            }
            for t in self.tiles[..usize::from(self.count)].iter_mut() {
                t.a = if t.a < 128 { t.a + 32 } else { 128 };
            }
            if !self.all {
                if self.count < 60 {
                    self.count += 1;
                } else {
                    self.all = true;
                }
            }
            if self.count == 60 {
                let s = f64::from(self.s);
                self.drawn.lib = Some((self.s, self.ring[0], self.glow));
                let ring = |r: f64, k: u32| {
                    let a = f64::from(0.209_439_5f32) * f64::from(k);
                    [r * a.cos(), r * a.sin(), 0.0]
                };
                let uv = uvn([[142, 64], [142, 0], [174, 64], [174, 0]], SHEET_256);
                for (outer, a) in [(155.0, self.ring[0]), (170.0, self.ring[1])] {
                    for k in 0..30 {
                        self.q(s, [ring(160.0, k), ring(outer, k), ring(160.0, k + 1), ring(outer, k + 1)], uv, [CYAN[0], CYAN[1], CYAN[2], a], 15);
                    }
                }
                self.shape1(130.0, s, [CYAN[0], CYAN[1], CYAN[2], self.glow], [176, 32], 32);
                let uv = uvn([[142, 0], [175, 0], [142, 63], [175, 63]], SHEET_256);
                for i in 0..64 {
                    let m = rotm64(self.rays[i][0], self.rays[i][1], self.rays[i][2]);
                    let w = RAY.map(|v| std::array::from_fn(|j| v[0] * m[0][j] + v[1] * m[1][j] + v[2] * m[2][j]));
                    self.q(s, w, uv, [CYAN[0], CYAN[1], CYAN[2], self.ray_a], 15);
                }
                if self.s <= 1.0 {
                    self.s += 0.2;
                } else {
                    self.bit1 = true;
                    self.s += 0.005;
                    self.ray_a = self.ray_a.saturating_sub(8);
                    self.ring = self.ring.map(|a| a.saturating_sub(8));
                    self.glow = self.glow.saturating_sub(10);
                    self.light = (self.light + 8).min(0x80);
                }
                for r in &mut self.rays {
                    *r = r.map(|v| v + 0x80);
                }
                if self.bit1 {
                    self.seal = self.seal.saturating_sub(8);
                    self.beam = self.beam.saturating_sub(8);
                    self.text_a = self.text_a.saturating_sub(16);
                }
            }
        }
        // 0x1F7F00(L): the board light.
        self.drawn_light = self.light;
        // SEAL (shape type 1, half 30, red).
        self.drawn.seal = self.seal;
        self.shape1(30.0, 1.0, [128, 0, 0, self.seal], [128, 128], 64);
        // BEAM (6-point shape, half 140).
        self.drawn.beam = self.beam;
        let xf = if self.beam_hw { 1.0 } else { 0.0 };
        let p = [[-140.0 * xf, 140.0], [-140.0 * xf, -140.0], [140.0 * xf, 140.0], [140.0 * xf, -140.0], [0.0, 140.0], [0.0, -140.0]].map(|[x, y]| [x, y, 0.0]);
        let uv = uvn([[128, 0], [128, 64], [0, 0], [0, 64]], SHEET_128X64);
        for k in 0..2 {
            self.q(1.0, [p[2 * k], p[4], p[2 * k + 1], p[5]], uv, [128, 128, 128, self.beam], 4);
        }
        if !self.bit0 {
            self.light = if self.light >= 0x20 + 8 { self.light - 8 } else { 0x20 };
            if self.seal < 0x40 {
                self.seal += 4;
            } else {
                self.seal = 0x40;
                self.bit0 = true;
            }
            self.beam = (self.beam + 8).min(0x80);
        }
        self.life -= 1;
        if self.life <= 0 {
            self.done = true;
            8
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(hw: bool) -> (C533, Vec<(u32, Drawn, u8, Vec<FxTri>)>) {
        let mut m = C533::with_beam(hw);
        let mut v = Vec::new();
        for u in 1..=UPDATES {
            let r = m.update(u);
            v.push((r, m.drawn(), m.board_light(), m.additive.clone()));
        }
        (m, v)
    }

    #[test]
    fn deterministic_init() {
        let m = C533::new();
        assert_eq!(m.rand_calls(), INIT_RAND);
        assert_eq!(m.frames(), FRAMES);
        assert_eq!(m, C533::new());
        let mut r = Rng::new(0);
        r.srand(59);
        for _ in 0..194 {
            r.next();
        }
        assert_eq!(game_rng_state_after_init(), r.state());
    }

    #[test]
    fn counts_returns_and_light() {
        let (mut m, v) = run(true);
        let at = |u: usize| &v[u - 1];
        assert!(v[..92].iter().all(|x| x.0 == 0));
        assert_eq!(at(93).0, 8);
        assert_eq!(m.update(94), 8);
        // Primitives 6 (u 1-17), 66 (u 18-76), 194 (u 77-93); 7,294 in all.
        for u in 1..=93usize {
            let want = if u <= 17 { 6 } else if u <= 76 { 66 } else { 194 };
            assert_eq!(at(u).1.prims, want, "u {u}");
            assert_eq!(at(u).3.len(), 2 * want);
        }
        assert_eq!(v.iter().map(|x| x.1.prims).sum::<usize>(), 7294);
        // Light: 128 - 8 (u - 1) to 32 at u 13, 32 to u 81, 32 + 8 (u - 81), 128 at u 93.
        for u in 1..=93usize {
            let want = if u <= 13 { 128 - 8 * (u - 1) } else if u <= 81 { 32 } else { 32 + 8 * (u - 81) };
            assert_eq!(usize::from(at(u).2), want, "u {u}");
        }
        assert_eq!((at(13).2, at(82).2, at(93).2), (32, 40, 128));
    }

    #[test]
    fn layer_alphas_and_scale() {
        let (_, v) = run(true);
        let d = |u: usize| v[u - 1].1;
        // SEAL 4 (u - 1) to 64 at u 17, 56 at u 82, 0 from u 89; BEAM 128 at u 17, 32 at u 93.
        assert_eq!((d(1).seal, d(17).seal, d(81).seal, d(82).seal, d(89).seal), (0, 64, 64, 56, 0));
        assert_eq!((d(1).beam, d(17).beam, d(81).beam, d(82).beam, d(93).beam), (0, 128, 128, 120, 32));
        // Tiles: tile 0 alpha 32 at u 20, tile 8 alpha 96 at u 30; descriptor 112 at u 83, 0 from u 90.
        assert_eq!((d(20).tile0, d(30).tile8, d(83).text, d(90).text), (32, 96, 112, 0));
        // Library: s 0.1 at u 77, 1.1 at u 82; glow 96 to u 82, 6 at u 91, 0 at u 92; rings 0 from u 86.
        let lib = |u: usize| d(u).lib.unwrap();
        assert!(d(76).lib.is_none());
        assert!((lib(77).0 - 0.1).abs() < 1e-6 && (lib(82).0 - 1.1).abs() < 1e-5);
        assert_eq!((lib(82).2, lib(91).2, lib(92).2), (96, 6, 0));
        assert_eq!((lib(82).1, lib(83).1, lib(85).1, lib(86).1), (32, 24, 8, 0));
    }

    #[test]
    fn screen_geometry_and_beam() {
        let (_, v) = run(true);
        // SEAL at u 1: x 226..286, y 215.69..296.25 px (scale 1, half 30).
        let seal: Vec<&FxTri> = v[0].3.iter().filter(|t| t.colour[0][..3] == [128, 0, 0]).collect();
        let xs = seal.iter().flat_map(|t| t.screen.iter().map(|p| p[0]));
        let (mn, mx) = xs.fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(x), b.max(x)));
        assert_eq!((mn, mx), (226.0, 286.0));
        let ys = seal.iter().flat_map(|t| t.screen.iter().map(|p| p[1]));
        let (mn, mx) = ys.fold((f32::MAX, f32::MIN), |(a, b), y| (a.min(y), b.max(y)));
        assert_eq!((mn, mx), (215.6875, 296.25));
        // BEAM on hardware: x 116..396, y 68..444 px; the interpreter's is zero-width.
        let beam: Vec<&FxTri> = v[0].3.iter().filter(|t| t.sheet == 4).collect();
        let (mn, mx) = beam.iter().flat_map(|t| t.screen.iter().map(|p| p[0])).fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(x), b.max(x)));
        assert_eq!((mn, mx), (116.0, 396.0));
        let (mn, mx) = beam.iter().flat_map(|t| t.screen.iter().map(|p| p[1])).fold((f32::MAX, f32::MIN), |(a, b), y| (a.min(y), b.max(y)));
        assert!((mn - 68.0).abs() < 0.07 && (mx - 444.0).abs() < 0.07, "{mn} {mx}");
        let (_, w) = run(false);
        assert!(w[0].3.iter().filter(|t| t.sheet == 4).all(|t| t.screen.iter().all(|p| p[0] == 256.0)));
        // TEXT tiles: 24 x 24 units at z +10 (24.31 x 32.65 px), TEXEFF 3.
        let t = v[17].3.iter().find(|t| t.sheet == 3).unwrap();
        assert!(((t.screen[1][0] - t.screen[0][0]) - 24.31).abs() < 0.07);
    }
}
