//! Two local pads (Claude, 5 Oct 2026): the original reads PS2 pad port 0
//! for control 100 and port 1 for control 101 (0x1CFF80: `pad = control −
//! 100`; Custom Duel's two-player menu 0x1C6FC0(side) and Trade's
//! 0x1C3650(side) read port `side`) [F]. The port maps local inputs to the
//! two ports here; Bevy-free so the mapping is tested in the library, the
//! binary's `pad_input.rs` feeds it from the keyboard and XInput gamepads.
//!
//! - Port 0 (pad 1): the duel's keyboard layout (`duel_play::KEYBOARD`:
//!   arrows, X Cross, A Square, Z Circle, D Triangle, S Start, Tab Select,
//!   Q/E L1/R1, 1 L2, W R2) plus C = L3 and V = R3 (Trade switches its
//!   list with L3/R3, mask 0x600 [F 0x1C3B64]), and the first gamepad.
//! - Port 1 (pad 2): [`KEYBOARD_PAD2`] (T/F/G/H d-pad, K Cross, J Square,
//!   L Circle, I Triangle, P Start, Y Select, U/O L1/R1, 7/9 L2/R2, B/N
//!   L3/R3) and the second gamepad.
//! - `ROSES_GAMEPAD_PADS` reassigns the gamepads: a comma list of ports
//!   (1 or 2) in gamepad order, e.g. `2` = the first gamepad is pad 2 (one
//!   player on the keyboard, one on a gamepad), `2,1` swaps two gamepads.
//!
//! Gamepads are the XInput controllers, then the Sony controllers read
//! through HID ([`sony`]: DualSense, DualShock 4), in connection order.
//! On the first port the gamepad also presses pad 1's keys (the binary's
//! `pad_input.rs`), so every scene that reads the keyboard follows it.
//!
//! Gamepad buttons by position (XInput): A Cross, B Circle, X Square, Y
//! Triangle, LB/RB L1/R1, LT/RT L2/R2 (past 30/255), Back Select, Start,
//! stick clicks L3/R3, d-pad; the sticks are the four analog bytes the
//! original reads with 0x203580(port, i): 0 right X, 1 right Y, 2 left X,
//! 3 left Y (0 = left / up, 0x80 = centre, 0xFF = right / down) [F: the
//! raw report's bytes 4..7 copied by 0x203230 to pad +6..+9].
//!
//! The pad word (Claude, 6 Oct 2026) [F 0x203230 / 0x203180]: each pad
//! read ORs `(raw[2] << 8 | raw[3]) ^ 0xFFFF` — the digital buttons only —
//! into pad +4, and the frame routine 0x203180 moves it to the current word
//! (+0, the previous to +2) and clears it, so a frame's word is every
//! button seen down since the last frame (a press shorter than a frame
//! still counts once). The sticks never set d-pad bits: 0x203230 copies
//! them to +6..+9 only, read by 0x203580 where a scene wants them (the
//! duel's camera 0x1D3CB8-0x1D3CF4, the deck editor's and Trade's
//! scrolling). Here: keyboard keys held plus keys tapped since the last
//! update, gamepads held plus buttons that went down since the last poll
//! ([`Latch`], [`GamepadSample::edges`]); no OS key repeat anywhere —
//! scenes apply the original's repeat counters (0x203380 kind 2) to the
//! word.

/// Pad bits as 0x203230 builds them (as `roses_core::deck_editor::pad`).
pub mod bit {
    pub const L2: u16 = 0x0001;
    pub const R2: u16 = 0x0002;
    pub const L1: u16 = 0x0004;
    pub const R1: u16 = 0x0008;
    pub const TRIANGLE: u16 = 0x0010;
    pub const CIRCLE: u16 = 0x0020;
    pub const CROSS: u16 = 0x0040;
    pub const SQUARE: u16 = 0x0080;
    pub const SELECT: u16 = 0x0100;
    pub const L3: u16 = 0x0200;
    pub const R3: u16 = 0x0400;
    pub const START: u16 = 0x0800;
    pub const UP: u16 = 0x1000;
    pub const RIGHT: u16 = 0x2000;
    pub const DOWN: u16 = 0x4000;
    pub const LEFT: u16 = 0x8000;
}

/// Pad 1's keys beyond the duel layout.
pub const KEYBOARD_PAD1_EXTRA: [(&str, u16); 2] = [("KeyC", bit::L3), ("KeyV", bit::R3)];

/// Pad 2's keyboard layout (Bevy `KeyCode` names).
pub const KEYBOARD_PAD2: [(&str, u16); 16] = [
    ("KeyT", bit::UP),
    ("KeyG", bit::DOWN),
    ("KeyF", bit::LEFT),
    ("KeyH", bit::RIGHT),
    ("KeyK", bit::CROSS),
    ("KeyJ", bit::SQUARE),
    ("KeyL", bit::CIRCLE),
    ("KeyI", bit::TRIANGLE),
    ("KeyP", bit::START),
    ("KeyY", bit::SELECT),
    ("KeyU", bit::L1),
    ("KeyO", bit::R1),
    ("Digit7", bit::L2),
    ("Digit9", bit::R2),
    ("KeyB", bit::L3),
    ("KeyN", bit::R3),
];

/// The bits a physical key gives each port (`[pad 1, pad 2]`).
pub fn key_pads(key_code: &str) -> [u16; 2] {
    let find = |table: &[(&str, u16)]| table.iter().find(|(n, _)| *n == key_code).map_or(0, |&(_, b)| b);
    [crate::duel_play::key_pad(key_code) | find(&KEYBOARD_PAD1_EXTRA), find(&KEYBOARD_PAD2)]
}

/// One line for the log: both layouts.
pub fn layout_note() -> String {
    "pads: pad 1 = arrows, X Cross, A Square, Z Circle, D Triangle, S Start, Tab Select, Q/E L1/R1, 1/W L2/R2, C/V L3/R3, first gamepad; pad 2 = T/F/G/H d-pad, K Cross, J Square, L Circle, I Triangle, P Start, Y Select, U/O L1/R1, 7/9 L2/R2, B/N L3/R3, second gamepad (ROSES_GAMEPAD_PADS=2: the first gamepad is pad 2)".into()
}

/// XInput's `XINPUT_GAMEPAD` fields → pad bits.
pub mod xinput {
    use super::bit;
    pub const DPAD_UP: u16 = 0x0001;
    pub const DPAD_DOWN: u16 = 0x0002;
    pub const DPAD_LEFT: u16 = 0x0004;
    pub const DPAD_RIGHT: u16 = 0x0008;
    pub const START: u16 = 0x0010;
    pub const BACK: u16 = 0x0020;
    pub const LEFT_THUMB: u16 = 0x0040;
    pub const RIGHT_THUMB: u16 = 0x0080;
    pub const LEFT_SHOULDER: u16 = 0x0100;
    pub const RIGHT_SHOULDER: u16 = 0x0200;
    pub const A: u16 = 0x1000;
    pub const B: u16 = 0x2000;
    pub const X: u16 = 0x4000;
    pub const Y: u16 = 0x8000;
    /// XINPUT_GAMEPAD_TRIGGER_THRESHOLD.
    pub const TRIGGER_THRESHOLD: u8 = 30;

    const MAP: [(u16, u16); 14] = [
        (DPAD_UP, bit::UP),
        (DPAD_DOWN, bit::DOWN),
        (DPAD_LEFT, bit::LEFT),
        (DPAD_RIGHT, bit::RIGHT),
        (START, bit::START),
        (BACK, bit::SELECT),
        (LEFT_THUMB, bit::L3),
        (RIGHT_THUMB, bit::R3),
        (LEFT_SHOULDER, bit::L1),
        (RIGHT_SHOULDER, bit::R1),
        (A, bit::CROSS),
        (B, bit::CIRCLE),
        (X, bit::SQUARE),
        (Y, bit::TRIANGLE),
    ];

    pub fn pad_bits(buttons: u16, left_trigger: u8, right_trigger: u8) -> u16 {
        let mut out = MAP.iter().filter(|(x, _)| buttons & x != 0).fold(0, |a, (_, p)| a | p);
        if left_trigger > TRIGGER_THRESHOLD {
            out |= bit::L2;
        }
        if right_trigger > TRIGGER_THRESHOLD {
            out |= bit::R2;
        }
        out
    }

    /// The thumb stick's Y (−32768..32767, up positive) as the PS2's
    /// analog byte (0 = up, 0x80 = centre, 0xFF = down).
    pub fn analog_byte(thumb_y: i16) -> u8 {
        (0x80 - (i32::from(thumb_y) >> 8)).clamp(0, 0xFF) as u8
    }

    /// A thumb stick's X (right positive) as the PS2's byte (0 = left).
    pub fn analog_x(thumb_x: i16) -> u8 {
        (0x80 + (i32::from(thumb_x) >> 8)).clamp(0, 0xFF) as u8
    }

    /// Both sticks as the PS2's four bytes (right X, right Y, left X,
    /// left Y).
    pub fn sticks(lx: i16, ly: i16, rx: i16, ry: i16) -> [u8; 4] {
        [analog_x(rx), analog_byte(ry), analog_x(lx), analog_byte(ly)]
    }
}

/// Sony controllers read as raw HID input reports (Windows has no XInput
/// for them): DualSense (USB report 0x01; Bluetooth 0x31 extended, or the
/// reduced 0x01 it sends until something switches it) and DualShock 4
/// (0x01 over USB or basic Bluetooth, 0x11 extended Bluetooth). Buttons by
/// position as XInput's: Cross, Circle, Square, Triangle, L1/R1, L2/R2
/// (the trigger's travel past 30/255, as XInput's threshold: the
/// controllers' own L2/R2 bits are set at the lightest touch, so a resting
/// finger held R2 — the duel's "show own cards" — and they are ignored),
/// Create/Share Select, Options Start, L3/R3, the hat as the d-pad; the
/// stick bytes (left X, left Y, right X, right Y in the report) are
/// already the PS2's (0 left / up, 0x80 centre). As in the original
/// (0x203230) the sticks do not set d-pad bits.
pub mod sony {
    use super::bit;

    pub const VENDOR: u16 = 0x054C;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Model {
        DualSense,
        DualShock4,
    }

    impl Model {
        pub fn name(self) -> &'static str {
            match self {
                Model::DualSense => "DualSense",
                Model::DualShock4 => "DualShock 4",
            }
        }
    }

    /// The model of a HID device by vendor/product id.
    pub fn model(vendor: u16, product: u16) -> Option<Model> {
        if vendor != VENDOR {
            return None;
        }
        match product {
            0x0CE6 | 0x0DF2 => Some(Model::DualSense),
            0x05C4 | 0x09CC | 0x0BA0 => Some(Model::DualShock4),
            _ => None,
        }
    }

    /// A Bluetooth connection: the device path names the Bluetooth HID
    /// service (00001124-…), or the input reports are longer than USB's 64.
    pub fn bluetooth(device_path: &str, input_report_len: usize) -> bool {
        device_path.to_ascii_lowercase().contains("00001124-0000-1000-8000-00805f9b34fb") || input_report_len > 64
    }

    /// Pad bits and the four stick bytes in the PS2's order (right X,
    /// right Y, left X, left Y) of one input report (report id first).
    pub fn parse(model: Model, bluetooth: bool, report: &[u8]) -> Option<(u16, [u8; 4])> {
        match (model, report.first()?) {
            // DualSense full layout: sticks, L2/R2 analog, counter, buttons.
            (Model::DualSense, 0x01) if !bluetooth => full(report, 1),
            (Model::DualSense, 0x31) => full(report, 2),
            // The reduced Bluetooth 0x01 and the DualShock 4 share a layout:
            // sticks, three button bytes, L2/R2 analog.
            (Model::DualSense, 0x01) | (Model::DualShock4, 0x01) => common(report, 1),
            (Model::DualShock4, 0x11) => common(report, 3),
            _ => None,
        }
    }

    /// Sticks at `o`, triggers at o+4/o+5, buttons at o+7/o+8.
    fn full(r: &[u8], o: usize) -> Option<(u16, [u8; 4])> {
        let b = r.get(o..o + 9)?;
        Some((buttons(b[7], b[8], b[4], b[5]), [b[2], b[3], b[0], b[1]]))
    }

    /// Sticks at `o`, buttons at o+4/o+5, triggers at o+7/o+8.
    fn common(r: &[u8], o: usize) -> Option<(u16, [u8; 4])> {
        let b = r.get(o..o + 9)?;
        Some((buttons(b[4], b[5], b[7], b[8]), [b[2], b[3], b[0], b[1]]))
    }

    /// Button byte 0 (hat in the low nibble; Square, Cross, Circle,
    /// Triangle in bits 4..7) and 1 (L1, R1, L2, R2, Create/Share,
    /// Options, L3, R3), and the triggers' travel.
    fn buttons(b0: u8, b1: u8, l2: u8, r2: u8) -> u16 {
        const HAT: [u16; 8] = [
            bit::UP,
            bit::UP | bit::RIGHT,
            bit::RIGHT,
            bit::DOWN | bit::RIGHT,
            bit::DOWN,
            bit::DOWN | bit::LEFT,
            bit::LEFT,
            bit::UP | bit::LEFT,
        ];
        const B0: [u16; 4] = [bit::SQUARE, bit::CROSS, bit::CIRCLE, bit::TRIANGLE];
        // L2/R2's own bits (2, 3) are left out: the travel decides.
        const B1: [u16; 8] = [bit::L1, bit::R1, 0, 0, bit::SELECT, bit::START, bit::L3, bit::R3];
        let mut out = HAT.get(usize::from(b0 & 0x0F)).copied().unwrap_or(0);
        for (i, &p) in B0.iter().enumerate() {
            if b0 & (0x10 << i) != 0 {
                out |= p;
            }
        }
        for (i, &p) in B1.iter().enumerate() {
            if b1 & (1 << i) != 0 {
                out |= p;
            }
        }
        if l2 > super::xinput::TRIGGER_THRESHOLD {
            out |= bit::L2;
        }
        if r2 > super::xinput::TRIGGER_THRESHOLD {
            out |= bit::R2;
        }
        out
    }
}

/// `ROSES_GAMEPAD_PADS`: the port (0/1) of each connected gamepad, in
/// gamepad order; default `[0, 1]`. Invalid entries are ignored.
pub fn gamepad_ports(var: Option<&str>) -> Vec<usize> {
    match var {
        Some(v) if !v.trim().is_empty() => v
            .split(',')
            .filter_map(|p| match p.trim() {
                "1" => Some(0),
                "2" => Some(1),
                _ => None,
            })
            .collect(),
        _ => vec![0, 1],
    }
}

/// One gamepad's state at a poll: the buttons down now, the buttons that
/// went down since the previous poll (a press and release between two
/// polls is in `edges` but not `held`), and its stick bytes (right X,
/// right Y, left X, left Y: 0x203580's indices).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GamepadSample {
    pub held: u16,
    pub edges: u16,
    pub analog: Option<[u8; 4]>,
}

/// Centred sticks.
pub const STICKS_CENTRED: [u8; 4] = [0x80; 4];

/// A gamepad read on its own thread (HID input reports arrive at 225 Hz
/// or more): every report updates `held` and adds its new presses to
/// `edges`; the frame takes both and the edges start again — the
/// counterpart of 0x203230 ORing every read into the pending word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Latch {
    pub held: u16,
    pub edges: u16,
    pub analog: [u8; 4],
}

impl Default for Latch {
    fn default() -> Self {
        Self { held: 0, edges: 0, analog: STICKS_CENTRED }
    }
}

impl Latch {
    /// One input report's buttons and stick bytes.
    pub fn report(&mut self, bits: u16, analog: [u8; 4]) {
        self.edges |= bits & !self.held;
        self.held = bits;
        self.analog = analog;
    }
    /// The state for this poll; the edges are consumed.
    pub fn take(&mut self) -> GamepadSample {
        let s = GamepadSample { held: self.held, edges: self.edges, analog: Some(self.analog) };
        self.edges = 0;
        s
    }
}

/// Key messages that make a keyboard reader follow a gamepad: `was` = the
/// buttons pressed as keys so far, `now` = this poll. Per button, in order:
/// a release, then a press when the button went down since the last poll
/// (or is down and was not), then a release when it is up again — so a
/// tap between two polls reaches the scenes as a press and a release in
/// one frame (their taps count it once), never as a key repeat.
pub fn key_transitions(was: u16, now: GamepadSample) -> Vec<(u16, bool)> {
    let mut out = Vec::new();
    for i in 0..16 {
        let b = 1u16 << i;
        let (w, h, e) = (was & b != 0, now.held & b != 0, now.edges & b != 0);
        let pressed = e || (h && !w);
        if w && (!h || pressed) {
            out.push((b, false));
        }
        if pressed {
            out.push((b, true));
            if !h {
                out.push((b, false));
            }
        }
    }
    out
}

/// `ROSES_PAD_TRACE=1`: log every 25 Hz update's pad words.
pub fn trace_enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("ROSES_PAD_TRACE").is_ok_and(|v| v == "1"))
}

/// The two ports' pad words from the keyboard (held keys and taps since
/// the last take) and the gamepads (held at the last poll and presses
/// since the last take).
#[derive(Clone, Debug, Default)]
pub struct TwoPads {
    /// Held keyboard bits per physical key, by port.
    keys: [std::collections::BTreeMap<String, u16>; 2],
    /// Presses seen since the last take (a tap shorter than an update
    /// still counts once).
    tapped: [u16; 2],
    /// Gamepad words by port at the last poll.
    pub gamepad: [u16; 2],
    /// Gamepad presses since the last take.
    gamepad_tapped: [u16; 2],
    /// Everything seen since the last `trace_word` (the trace does not
    /// consume the scenes' taps).
    traced: [u16; 2],
    /// The stick bytes of the gamepad on each port, if any.
    pub analog: [Option<[u8; 4]>; 2],
}

impl TwoPads {
    pub fn key_down(&mut self, key_code: &str) {
        for (p, bits) in key_pads(key_code).into_iter().enumerate() {
            if bits != 0 {
                self.keys[p].insert(key_code.to_string(), bits);
                self.tapped[p] |= bits;
                self.traced[p] |= bits;
            }
        }
    }
    pub fn key_up(&mut self, key_code: &str) {
        for k in &mut self.keys {
            k.remove(key_code);
        }
    }
    pub fn release_all(&mut self) {
        for k in &mut self.keys {
            k.clear();
        }
    }
    /// Gamepad `index` (connection order) as polled this frame.
    pub fn set_gamepads(&mut self, samples: &[GamepadSample], ports: &[usize]) {
        self.gamepad = [0; 2];
        self.analog = [None; 2];
        for (i, s) in samples.iter().enumerate() {
            if let Some(&p) = ports.get(i).filter(|&&p| p < 2) {
                self.gamepad[p] |= s.held;
                self.gamepad_tapped[p] |= s.edges;
                self.traced[p] |= s.held | s.edges;
                if self.analog[p].is_none() {
                    self.analog[p] = s.analog;
                }
            }
        }
    }
    /// Port `p`'s keyboard bits (held | tapped); the taps are consumed.
    pub fn take_keys(&mut self, p: usize) -> u16 {
        let held = self.keys[p].values().fold(self.tapped[p], |a, &b| a | b);
        self.tapped[p] = 0;
        held
    }
    /// Port `p`'s gamepad bits (held | pressed since the last take).
    pub fn take_gamepad(&mut self, p: usize) -> u16 {
        let w = self.gamepad[p] | self.gamepad_tapped[p];
        self.gamepad_tapped[p] = 0;
        w
    }
    /// Port `p`'s whole word (keyboard and gamepad).
    pub fn take(&mut self, p: usize) -> u16 {
        self.take_keys(p) | self.take_gamepad(p)
    }
    /// Port `p`'s word for the trace: as `take` would give it, but only
    /// the trace's own record of taps is consumed.
    pub fn trace_word(&mut self, p: usize) -> u16 {
        let w = self.keys[p].values().fold(self.traced[p] | self.gamepad[p], |a, &b| a | b);
        self.traced[p] = 0;
        w
    }
    /// Forget taps (a scene starts).
    pub fn clear_taps(&mut self) {
        self.tapped = [0; 2];
        self.gamepad_tapped = [0; 2];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_layouts_do_not_overlap() {
        assert_eq!(key_pads("ArrowUp"), [bit::UP, 0]);
        assert_eq!(key_pads("KeyT"), [0, bit::UP]);
        assert_eq!(key_pads("KeyK"), [0, bit::CROSS]);
        assert_eq!(key_pads("KeyC"), [bit::L3, 0]);
        assert_eq!(key_pads("KeyN"), [0, bit::R3]);
        for (k, _) in KEYBOARD_PAD2 {
            assert_eq!(crate::duel_play::key_pad(k), 0, "{k} is a pad-1 key");
        }
        let mut seen = 0u16;
        for (_, b) in KEYBOARD_PAD2 {
            assert_eq!(seen & b, 0);
            seen |= b;
        }
        assert_eq!(seen, 0xFFFF, "pad 2 has every button");
    }

    #[test]
    fn xinput_buttons_map_by_position() {
        use xinput::*;
        assert_eq!(pad_bits(A | DPAD_LEFT, 0, 0), bit::CROSS | bit::LEFT);
        assert_eq!(pad_bits(B | Y | X, 0, 255), bit::CIRCLE | bit::TRIANGLE | bit::SQUARE | bit::R2);
        assert_eq!(pad_bits(START | BACK | LEFT_THUMB, 31, 30), bit::START | bit::SELECT | bit::L3 | bit::L2);
        assert_eq!(analog_byte(0), 0x80);
        assert_eq!(analog_byte(i16::MAX), 1);
        assert_eq!(analog_byte(i16::MIN), 0xFF);
        assert_eq!(analog_x(i16::MIN), 0);
        assert_eq!(analog_x(i16::MAX), 0xFF);
        // Left stick pushed right, right stick pushed up.
        assert_eq!(sticks(i16::MAX, 0, 0, i16::MAX), [0x80, 1, 0xFF, 0x80]);
    }

    #[test]
    fn sony_reports_parse_in_every_layout() {
        use sony::*;
        // DualSense over USB (0x01, 64 bytes): Cross + hat E, L1, Options,
        // left stick up, right stick right and down, R2 half way (analog
        // only).
        let mut usb = [0u8; 64];
        usb[0] = 0x01;
        usb[1..5].copy_from_slice(&[0x80, 0x10, 0xF0, 0xC0]);
        usb[6] = 0x90; // R2 travel
        usb[8] = 0x20 | 2; // Cross, hat E
        usb[9] = 0x01 | 0x20; // L1, Options
        assert_eq!(
            parse(Model::DualSense, false, &usb),
            Some((bit::CROSS | bit::RIGHT | bit::L1 | bit::START | bit::R2, [0xF0, 0xC0, 0x80, 0x10]))
        );
        // The same state over Bluetooth's extended 0x31 (one byte later).
        let mut bt = [0u8; 78];
        bt[0] = 0x31;
        bt[1] = 0x42;
        bt[2..65].copy_from_slice(&usb[1..64]);
        assert_eq!(parse(Model::DualSense, true, &bt), parse(Model::DualSense, false, &usb));
        // Bluetooth's reduced 0x01 (padded to the 78-byte buffer): hat
        // none, Triangle + Square, Create, R3; L2 by its travel, R2's
        // digital bit at a light touch (travel 20) is not R2.
        let mut simple = [0u8; 78];
        simple[0] = 0x01;
        simple[1..5].copy_from_slice(&[0x80, 0xFF, 0x80, 0x80]);
        simple[5] = 0x80 | 0x10 | 8;
        simple[6] = 0x10 | 0x80 | 0x04 | 0x08;
        simple[8] = 31;
        simple[9] = 20;
        assert_eq!(
            parse(Model::DualSense, true, &simple),
            Some((bit::TRIANGLE | bit::SQUARE | bit::SELECT | bit::R3 | bit::L2, [0x80, 0x80, 0x80, 0xFF]))
        );
        simple[8] = 30;
        assert_eq!(parse(Model::DualSense, true, &simple), Some((bit::TRIANGLE | bit::SQUARE | bit::SELECT | bit::R3, [0x80, 0x80, 0x80, 0xFF])));
        // DualShock 4: USB 0x01 and Bluetooth 0x11 (two more header bytes).
        let mut ds4 = [0u8; 64];
        ds4[0] = 0x01;
        ds4[1..5].copy_from_slice(&[0x80, 0x80, 0x80, 0x80]);
        ds4[5] = 0x40 | 7; // Circle, hat NW
        ds4[6] = 0x02 | 0x40; // R1, L3
        ds4[8] = 200; // L2 travel
        let want = Some((bit::CIRCLE | bit::UP | bit::LEFT | bit::R1 | bit::L3 | bit::L2, STICKS_CENTRED));
        assert_eq!(parse(Model::DualShock4, false, &ds4), want);
        assert_eq!(parse(Model::DualShock4, true, &ds4), want);
        let mut ds4bt = vec![0u8; 547];
        ds4bt[0] = 0x11;
        ds4bt[1] = 0xC0;
        ds4bt[3..12].copy_from_slice(&ds4[1..10]);
        assert_eq!(parse(Model::DualShock4, true, &ds4bt), want);
        // Idle, unknown reports and short reads.
        let mut idle = [0u8; 64];
        idle[0] = 0x01;
        idle[8] = 8;
        idle[1..5].copy_from_slice(&STICKS_CENTRED);
        assert_eq!(parse(Model::DualSense, false, &idle), Some((0, STICKS_CENTRED)));
        assert_eq!(parse(Model::DualSense, false, &[0x05, 1, 2]), None);
        assert_eq!(parse(Model::DualSense, true, &[0x31, 0, 0x80]), None);
        assert_eq!(model(0x054C, 0x0CE6), Some(Model::DualSense));
        assert_eq!(model(0x054C, 0x09CC), Some(Model::DualShock4));
        assert_eq!(model(0x045E, 0x0CE6), None);
        assert!(bluetooth(r"\\?\HID#{00001124-0000-1000-8000-00805F9B34FB}_VID&0002054C_PID&0CE6#x", 78));
        assert!(!bluetooth(r"\\?\HID#VID_054C&PID_0CE6&MI_03#x", 64));
    }

    #[test]
    fn two_pads_take_taps_and_gamepads() {
        let mut p = TwoPads::default();
        p.key_down("KeyX");
        p.key_up("KeyX");
        p.key_down("KeyH");
        let start = GamepadSample { held: bit::START, edges: bit::START, analog: Some(STICKS_CENTRED) };
        p.set_gamepads(&[start], &gamepad_ports(Some("2")));
        assert_eq!(p.take(0), bit::CROSS);
        assert_eq!(p.take(0), 0);
        assert_eq!(p.take(1), bit::RIGHT | bit::START);
        assert_eq!(p.analog, [None, Some(STICKS_CENTRED)]);
        assert_eq!(gamepad_ports(None), vec![0, 1]);
        assert_eq!(gamepad_ports(Some("2,1")), vec![1, 0]);
        // A gamepad tap between two polls (edge, not held) counts once.
        let tap = GamepadSample { held: 0, edges: bit::CROSS, analog: None };
        p.set_gamepads(&[tap], &[0]);
        p.set_gamepads(&[GamepadSample::default()], &[0]);
        assert_eq!(p.trace_word(0), bit::CROSS);
        assert_eq!(p.take(0), bit::CROSS);
        assert_eq!(p.take(0), 0);
        assert_eq!(p.trace_word(0), 0);
    }

    #[test]
    fn latch_keeps_presses_between_polls() {
        let mut l = Latch::default();
        // Cross down and up again between two polls (HID reports at 225 Hz).
        let (c, up) = (STICKS_CENTRED, [0x80, 0x80, 0x80, 0x10]);
        l.report(bit::CROSS, c);
        l.report(0, c);
        assert_eq!(l.take(), GamepadSample { held: 0, edges: bit::CROSS, analog: Some(c) });
        assert_eq!(l.take().edges, 0);
        // Held across polls: one edge only.
        l.report(bit::RIGHT, up);
        l.report(bit::RIGHT, up);
        assert_eq!(l.take(), GamepadSample { held: bit::RIGHT, edges: bit::RIGHT, analog: Some(up) });
        l.report(bit::RIGHT, up);
        assert_eq!(l.take(), GamepadSample { held: bit::RIGHT, edges: 0, analog: Some(up) });
        // Released and pressed again between polls: a new edge while held.
        l.report(0, c);
        l.report(bit::RIGHT, c);
        assert_eq!(l.take().edges, bit::RIGHT);
    }

    #[test]
    fn key_transitions_never_repeat_and_keep_taps() {
        let s = |held, edges| GamepadSample { held, edges, analog: None };
        let (x, r) = (bit::CROSS, bit::RIGHT);
        assert_eq!(key_transitions(0, s(r, r)), [(r, true)]);
        // Held: nothing (no key repeat), however long.
        assert!(key_transitions(r, s(r, 0)).is_empty());
        assert_eq!(key_transitions(r, s(0, 0)), [(r, false)]);
        // A tap between polls: press and release in the same frame.
        assert_eq!(key_transitions(0, s(0, x)), [(x, true), (x, false)]);
        // Released and pressed again between polls: release, press.
        assert_eq!(key_transitions(r, s(r, r)), [(r, false), (r, true)]);
        // XInput (no edges of its own) still presses on a new hold.
        assert_eq!(key_transitions(0, s(x | r, 0)), [(x, true), (r, true)]);
    }
}
