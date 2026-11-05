//! The leaders' appearance effect at the duel's start, CMODULE c620
//! (program 0x26C, `0x1F5B00(0, 0x26C, 0, cell, 0)` per side in input state
//! 0xB, 0x1D0638), hand-ported to plain Rust (`roses_fx::c620`, equal to
//! the original update for update, bit for bit: `roses_ee::parity`, the
//! test oracle, not linked into the game), with the board camera mirrored; its GS primitives
//! are drawn over the board. Bevy-free.
//!
//! Source [F]: the module reads only its target (0x1F6590: row << 8 |
//! column), the cell's position (0x1E2370), TEXEFF record 15 (0x103260) and
//! the camera (0x20E2B0 / 0x1FC1C0 through 0x1038D0 / 0x1039A0); it returns
//! 2 on its 39th update (the leader is shown, 0x1E2E90) and 8 on its 72nd
//! [emu: `modes/older/c620/run1.jsonl`, the EE interpreter from the PCSX2
//! state of update 123 (`tools/c620_run.py`); PCSX2: status 2 at u129 = the
//! launch u90 + 39].
//!
//! [H]: rand starts from the executable's state (the game's depends on the
//! session); the launch's init and first update run in the launch update.
use crate::duel_bmodule::guarded;
use roses_fx::EffectPort;
use roses_fx::types::{CameraMirror, EffectTextures, EffectWorld, GsPrim, UnitMirror};
use std::sync::Arc;

/// CMODULE program of the leaders' appearance.
pub const C620: u16 = 0x26C;

/// The board as c620 sees it: the camera only.
pub struct BoardWorld {
    pub camera: CameraMirror,
}

impl EffectWorld for BoardWorld {
    fn camera(&self) -> CameraMirror {
        self.camera
    }
    fn unit(&self, _role: usize) -> UnitMirror {
        UnitMirror {
            position: [0.0; 3],
            rotation: [0; 3],
            animated: [0.0; 3],
            motion: 0,
            time: 0.0,
            speed: 1.0,
            play_mode: 0,
            frozen: false,
            frames: [0; 8],
            alpha: 0x80,
            nodes: 0,
            model: 0,
            bounds: [0.0; 4],
        }
    }
    fn node(&self, _role: usize, _node: u8) -> Option<[f32; 3]> {
        None
    }
}

/// The board camera for the runner: the board's look-at and its GS
/// projection (distance 800, `duel_board::GS_PROJECTION`).
pub fn board_camera(eye: [f64; 3], target: [f64; 3]) -> CameraMirror {
    CameraMirror { eye: eye.map(|v| v as f32), target: target.map(|v| v as f32), distance: 800.0, roll: 0.0 }
}

/// One side's running c620.
pub struct LeaderEffect {
    /// Behind a mutex: a port is `Send`, not `Sync` by contract (the duel
    /// play is a Bevy resource).
    effect: std::sync::Mutex<roses_fx::c620::C620>,
    /// The port panicked (`duel_bmodule::caught`).
    stopped: bool,
    /// The last update's return (2 once the leader shows, 8 at the end).
    pub ret: i32,
    /// Updates run after the launch (the launch's own update is 1).
    pub updates: u32,
    /// The last update's primitives.
    pub prims: Arc<Vec<GsPrim>>,
}

impl LeaderEffect {
    /// Launch on `cell` (column, row): the init call, then the launch
    /// update's first update with `camera`. `None` without the program.
    pub fn launch(executable: &[u8], cmodule: &[u8], texeff: Option<&[u8]>, cell: (u8, u8), camera: CameraMirror) -> Option<Self> {
        let textures = EffectTextures { texeff: texeff.map(crate::duel_bmodule::texeff_records).unwrap_or_default(), own: Vec::new(), other: Vec::new() };
        let target = (u16::from(cell.1) << 8) | u16::from(cell.0);
        let effect = guarded("c620: the leader effect", || roses_fx::c620::C620::load(executable, cmodule, [0, target], &textures)).ok()??;
        let mut fx = Self { effect: std::sync::Mutex::new(effect), stopped: false, ret: 0, updates: 0, prims: Arc::new(Vec::new()) };
        let world = BoardWorld { camera };
        let effect = fx.effect.get_mut().ok()?;
        guarded("c620: the leader effect", || effect.update(&world)).ok()?;
        fx.update(camera);
        Some(fx)
    }

    /// One update with this update's camera; false once it has ended.
    pub fn update(&mut self, camera: CameraMirror) -> bool {
        let Ok(effect) = self.effect.get_mut() else { return false };
        if self.stopped || effect.finished() {
            self.prims = Arc::new(Vec::new());
            return false;
        }
        match guarded("c620: the leader effect", || effect.update(&BoardWorld { camera })) {
            Ok(u) => {
                self.ret = u.ret;
                self.updates += 1;
                self.prims = Arc::new(u.prims);
                true
            }
            Err(_) => {
                self.stopped = true;
                self.prims = Arc::new(Vec::new());
                false
            }
        }
    }

    pub fn finished(&self) -> bool {
        self.stopped || self.effect.lock().map_or(true, |e| e.finished())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// c620 against the EE interpreter's run from the PCSX2 state at u123
    /// (camera eye (0, 95.557, 66.853), target (0, 40, 150)): returns 2 on
    /// update 39 and 8 on update 72; the deterministic layers' primitive
    /// counts (vignette 4, ground fan 36 then 72 from update 20) [emu
    /// `modes/older/c620/run1.jsonl`].
    #[test]
    #[ignore = "requires ROSES_DUEL_IMPORT and ROSES_CMODULE (CMODULE.MRG)"]
    fn private_c620_runs_like_the_interpreter() {
        let duel = std::path::PathBuf::from(std::env::var_os("ROSES_DUEL_IMPORT").expect("ROSES_DUEL_IMPORT"));
        let cmodule = std::fs::read(std::env::var_os("ROSES_CMODULE").expect("ROSES_CMODULE")).unwrap();
        let data = crate::duel_play::load(&duel).unwrap();
        let cam = board_camera([0.0, 95.557, 66.853], [0.0, 40.0, 150.0]);
        let started = std::time::Instant::now();
        let mut fx = LeaderEffect::launch(&data.executable, &cmodule, data.texeff.as_deref(), (3, 6), cam).expect("c620");
        let mut rets = vec![fx.ret];
        let mut counts = vec![fx.prims.len()];
        while fx.update(cam) {
            rets.push(fx.ret);
            counts.push(fx.prims.len());
            assert!(rets.len() < 200);
        }
        eprintln!("c620: {} updates in {:?}; counts {counts:?}", rets.len(), started.elapsed());
        assert_eq!(rets.iter().position(|&r| r == 2), Some(38), "ret 2 on update 39");
        assert_eq!(rets.len(), 72);
        assert_eq!(*rets.last().unwrap(), 8);
        assert_eq!(counts[0], 288);
        assert!(counts[38] > 450);
    }
}
