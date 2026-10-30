//! The two local pads as a resource (`roses_game::pads`):
//! keyboard messages every frame, gamepads polled every frame — XInput
//! controllers (Windows; loaded at run time from xinput1_4.dll /
//! xinput9_1_0.dll) and Sony DualSense / DualShock 4 controllers read as
//! raw HID input reports on reader threads (`hid` below; nothing to
//! install). Scenes that read pad 2 (two-player Custom Duel, a duel with
//! two humans, Trade) take the words once per 25 Hz update.
//!
//! The first port's gamepad also presses pad 1's keys (the duel layout:
//! arrows, X, A, Z, D, S, Tab, Q/E, 1/W, C/V) as keyboard messages, so the
//! title, story, map, deck editor, slot machine and every other scene that
//! reads the keyboard follows the gamepad too. Gamepads count only while
//! the game window has the focus.
//!
//! Feel (Claude, 6 Oct 2026): a gamepad gives exactly what the keys give —
//! a press, a release, and for a press shorter than a poll (the HID reader
//! keeps every report's new presses, [`pads::Latch`]) a press and release
//! in one frame, which the scenes' taps count once, like the original's
//! ORed pad word [F 0x203230]. No key repeat is synthesised (and the OS's
//! key repeat is ignored by every scene): each scene derives its presses
//! and auto-repeat from the held word per 25 Hz update with the original's
//! counters (0x203180 / 0x203380 kind 2: first repeat 13 updates after the
//! press, then every update; pad +0x59 = 12, +0x5A & 0xF = 1).
//! `ROSES_PAD_TRACE=1` logs every update's pad words and each gamepad key
//! message with its time.
use bevy::{
    input::{
        ButtonState, InputSystems,
        keyboard::{Key, KeyboardFocusLost, KeyboardInput},
    },
    prelude::*,
    window::PrimaryWindow,
};
use roses_game::pads::{self, TwoPads, bit};
use std::time::Instant;

#[derive(Resource)]
pub struct Pads {
    pub pads: TwoPads,
    ports: Vec<usize>,
    xinput: Option<xinput::XInput>,
    hid: hid::Sony,
    connected: usize,
    /// Pad 1's buttons pressed as keys so far.
    keyed: u16,
    /// Tests: these samples stand in for the controllers (edges are
    /// consumed by the poll).
    pub(crate) inject: Option<Vec<pads::GamepadSample>>,
    /// `ROSES_PAD_TRACE`: start time, update count, last words.
    started: Instant,
    traced: u64,
    trace_last: [u16; 2],
}

impl Default for Pads {
    fn default() -> Self {
        let xinput = xinput::XInput::load();
        Self {
            pads: TwoPads::default(),
            ports: pads::gamepad_ports(std::env::var("ROSES_GAMEPAD_PADS").ok().as_deref()),
            xinput,
            hid: hid::Sony::start(),
            connected: usize::MAX,
            keyed: 0,
            inject: None,
            started: Instant::now(),
            traced: 0,
            trace_last: [0; 2],
        }
    }
}

impl Pads {
    /// Port `p`'s word (keyboard held | taps since the last take | gamepad).
    pub fn take(&mut self, p: usize) -> u16 {
        self.pads.take(p)
    }
    /// What pad 1 adds to a scene that reads its own keyboard layout (the
    /// duel's keys): the L3/R3 keys C/V (the taps of the port are
    /// consumed). The first port's gamepad already reaches such a scene as
    /// its keys — adding its held word here as well made a button still
    /// down from the previous scene a fresh press on the scene's first
    /// update (the keys' first read drops such carried presses), and was
    /// the only place the gamepad and the keyboard differed.
    pub fn pad1_extra(&mut self) -> u16 {
        let keys = self.pads.take_keys(0);
        self.pads.take_gamepad(0);
        let extra = pads::KEYBOARD_PAD1_EXTRA.iter().fold(0, |a, &(_, b)| a | b);
        keys & extra
    }
    /// No controllers: only `inject` (tests).
    #[cfg(test)]
    pub(crate) fn without_controllers() -> Self {
        Self {
            pads: TwoPads::default(),
            ports: vec![0, 1],
            xinput: None,
            hid: hid::Sony::default(),
            connected: 0,
            keyed: 0,
            inject: Some(Vec::new()),
            started: Instant::now(),
            traced: 0,
            trace_last: [0; 2],
        }
    }
    /// Port `p`'s left stick Y (0x203580(p, 3)), with an analog pad.
    pub fn analog(&self, p: usize) -> Option<u8> {
        self.sticks(p).map(|s| s[3])
    }
    /// Port `p`'s four stick bytes (0x203580(p, 0..3)), with an analog pad
    /// (0x203500(p) != 0); `None` for the keyboard alone.
    pub fn sticks(&self, p: usize) -> Option<[u8; 4]> {
        self.pads.analog.get(p).copied().flatten()
    }
}

/// The pads for every mode (idempotent: a second call adds nothing).
pub fn add_systems(app: &mut App) {
    if app.world().contains_resource::<Pads>() {
        return;
    }
    app.init_resource::<Pads>()
        .add_systems(PreUpdate, (poll_gamepads.before(InputSystems), collect.after(poll_gamepads)))
        .add_systems(FixedUpdate, trace_updates.run_if(|| pads::trace_enabled()));
}

/// `ROSES_PAD_TRACE=1`: each 25 Hz update's words (pad 1 and 2 as the
/// scenes take them: held keys and buttons plus presses since the last
/// update), logged while anything is held or changed.
fn trace_updates(mut pads: ResMut<Pads>) {
    let p = &mut *pads;
    p.traced += 1;
    let w = [p.pads.trace_word(0), p.pads.trace_word(1)];
    if w != [0, 0] || w != p.trace_last {
        eprintln!(
            "ROSES_PAD_TRACE update {} at {:.1} ms: pad1 {:#06x} pad2 {:#06x} (gamepads {:#06x}/{:#06x}, sticks RX RY LX LY {:02x?}/{:02x?})",
            p.traced,
            p.started.elapsed().as_secs_f64() * 1000.0,
            w[0],
            w[1],
            p.pads.gamepad[0],
            p.pads.gamepad[1],
            p.pads.analog[0],
            p.pads.analog[1]
        );
    }
    p.trace_last = w;
}

/// Pad 1's buttons as the keys of the duel layout (`duel_play::KEYBOARD`
/// plus C/V for L3/R3): physical key and its logical key.
fn pad1_keys() -> [(u16, KeyCode, Key); 16] {
    let c = |s: &str| Key::Character(s.into());
    [
        (bit::UP, KeyCode::ArrowUp, Key::ArrowUp),
        (bit::DOWN, KeyCode::ArrowDown, Key::ArrowDown),
        (bit::LEFT, KeyCode::ArrowLeft, Key::ArrowLeft),
        (bit::RIGHT, KeyCode::ArrowRight, Key::ArrowRight),
        (bit::CROSS, KeyCode::KeyX, c("x")),
        (bit::SQUARE, KeyCode::KeyA, c("a")),
        (bit::CIRCLE, KeyCode::KeyZ, c("z")),
        (bit::TRIANGLE, KeyCode::KeyD, c("d")),
        (bit::START, KeyCode::KeyS, c("s")),
        (bit::SELECT, KeyCode::Tab, Key::Tab),
        (bit::L1, KeyCode::KeyQ, c("q")),
        (bit::R1, KeyCode::KeyE, c("e")),
        (bit::L2, KeyCode::Digit1, c("1")),
        (bit::R2, KeyCode::KeyW, c("w")),
        (bit::L3, KeyCode::KeyC, c("c")),
        (bit::R3, KeyCode::KeyV, c("v")),
    ]
}

/// Poll the gamepads (before the keyboard messages are read this frame)
/// and press / release pad 1's keys for the first port's gamepad.
fn poll_gamepads(
    mut pads: ResMut<Pads>,
    windows: Query<(Entity, &Window), With<PrimaryWindow>>,
    mut keys: MessageWriter<KeyboardInput>,
) {
    let mut samples = match pads.inject.as_mut() {
        Some(inject) => {
            let out = inject.clone();
            inject.iter_mut().for_each(|s| s.edges = 0);
            out
        }
        None => {
            let mut samples = pads.xinput.as_mut().map(|x| x.poll()).unwrap_or_default();
            samples.extend(pads.hid.poll());
            if samples.len() != pads.connected {
                pads.connected = samples.len();
                eprintln!("Pads: {} gamepad(s) connected; {}", samples.len(), pads::layout_note());
            }
            samples
        }
    };
    let window = windows.iter().next();
    if !window.is_some_and(|(_, w)| w.focused) {
        samples.iter_mut().for_each(|s| (s.held, s.edges) = (0, 0));
    }
    let ports = pads.ports.clone();
    pads.pads.set_gamepads(&samples, &ports);
    let Some((window, _)) = window else {
        return;
    };
    // Port 0's gamepads as one sample: the keys pad 1 presses.
    let first = samples
        .iter()
        .zip(&ports)
        .filter(|&(_, &p)| p == 0)
        .fold(pads::GamepadSample::default(), |a, (s, _)| pads::GamepadSample { held: a.held | s.held, edges: a.edges | s.edges, analog: a.analog.or(s.analog) });
    let table = pad1_keys();
    for (b, pressed) in pads::key_transitions(pads.keyed, first) {
        let Some((_, key_code, logical_key)) = table.iter().find(|(k, _, _)| *k == b).cloned() else {
            continue;
        };
        if pads::trace_enabled() {
            eprintln!(
                "ROSES_PAD_TRACE frame at {:.1} ms: gamepad {key_code:?} {}",
                pads.started.elapsed().as_secs_f64() * 1000.0,
                if pressed { "down" } else { "up" }
            );
        }
        let state = if pressed { ButtonState::Pressed } else { ButtonState::Released };
        keys.write(KeyboardInput { key_code, logical_key, state, text: None, repeat: false, window });
    }
    pads.keyed = first.held;
}

fn collect(mut events: MessageReader<KeyboardInput>, mut focus: MessageReader<KeyboardFocusLost>, mut pads: ResMut<Pads>) {
    for e in events.read() {
        let name = format!("{:?}", e.key_code);
        match e.state {
            ButtonState::Pressed if !e.repeat => pads.pads.key_down(&name),
            ButtonState::Pressed => {}
            ButtonState::Released => pads.pads.key_up(&name),
        }
    }
    if focus.read().count() > 0 {
        pads.pads.release_all();
    }
}

#[cfg(windows)]
mod xinput {
    use roses_game::pads::{GamepadSample, xinput as map};
    use std::ffi::c_void;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[repr(C)]
    #[derive(Default)]
    struct Gamepad {
        buttons: u16,
        left_trigger: u8,
        right_trigger: u8,
        thumb_lx: i16,
        thumb_ly: i16,
        thumb_rx: i16,
        thumb_ry: i16,
    }
    #[repr(C)]
    #[derive(Default)]
    struct State {
        packet: u32,
        gamepad: Gamepad,
    }
    type GetState = unsafe extern "system" fn(u32, *mut State) -> u32;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LoadLibraryA(name: *const u8) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    }

    pub struct XInput {
        get_state: GetState,
        /// Ports that answered. Empty ports are asked again every two
        /// seconds on a thread of their own (asking an empty port takes
        /// long enough to hitch a frame).
        live: Arc<[AtomicBool; 4]>,
        last: [u16; 4],
    }

    impl XInput {
        pub fn load() -> Option<Self> {
            for dll in [&b"xinput1_4.dll\0"[..], b"xinput1_3.dll\0", b"xinput9_1_0.dll\0"] {
                // SAFETY: NUL-terminated names; the returned procedure has
                // XInputGetState's documented signature.
                let get_state = unsafe {
                    let module = LoadLibraryA(dll.as_ptr());
                    if module.is_null() {
                        continue;
                    }
                    let f = GetProcAddress(module, b"XInputGetState\0".as_ptr());
                    if f.is_null() {
                        continue;
                    }
                    std::mem::transmute::<*mut c_void, GetState>(f)
                };
                let live: Arc<[AtomicBool; 4]> = Arc::new(Default::default());
                let scan = live.clone();
                let spawned = std::thread::Builder::new().name("roses-xinput-scan".into()).spawn(move || {
                    loop {
                        for user in 0..4u32 {
                            if !scan[user as usize].load(Ordering::Relaxed) {
                                let mut s = State::default();
                                // SAFETY: `s` is a valid XINPUT_STATE; XInputGetState is thread-safe.
                                if unsafe { get_state(user, &mut s) } == 0 {
                                    scan[user as usize].store(true, Ordering::Relaxed);
                                }
                            }
                        }
                        std::thread::sleep(std::time::Duration::from_secs(2));
                    }
                });
                if let Err(e) = spawned {
                    eprintln!("Pads: the XInput scan did not start: {e}");
                    return None;
                }
                return Some(Self { get_state, live, last: [0; 4] });
            }
            None
        }

        /// Connected controllers in user-index order.
        pub fn poll(&mut self) -> Vec<GamepadSample> {
            let mut out = Vec::new();
            for user in 0..4usize {
                if !self.live[user].load(Ordering::Relaxed) {
                    self.last[user] = 0;
                    continue;
                }
                let mut s = State::default();
                // SAFETY: `s` is a valid XINPUT_STATE for the call.
                if unsafe { (self.get_state)(user as u32, &mut s) } != 0 {
                    self.live[user].store(false, Ordering::Relaxed);
                    self.last[user] = 0;
                    continue;
                }
                let g = &s.gamepad;
                let held = map::pad_bits(g.buttons, g.left_trigger, g.right_trigger);
                let sticks = map::sticks(g.thumb_lx, g.thumb_ly, g.thumb_rx, g.thumb_ry);
                out.push(GamepadSample { held, edges: held & !self.last[user], analog: Some(sticks) });
                self.last[user] = held;
            }
            out
        }
    }
}

#[cfg(not(windows))]
mod xinput {
    use roses_game::pads::GamepadSample;
    pub struct XInput;
    impl XInput {
        pub fn load() -> Option<Self> {
            None
        }
        pub fn poll(&mut self) -> Vec<GamepadSample> {
            Vec::new()
        }
    }
}

/// Sony controllers through the Windows HID API: a scanner thread lists
/// the HID gamepads every two seconds (SetupDi + HidD), opens each
/// DualSense / DualShock 4 not yet open and gives it a reader thread that
/// blocks in ReadFile and keeps the latest parsed state; the frame only
/// reads that state. No feature report is sent, so the controller stays in
/// the mode it is in (a Bluetooth DualSense sends the reduced 0x01 report
/// until something switches it to 0x31; both are read).
#[cfg(windows)]
mod hid {
    use roses_game::pads::sony::{self, Model};
    use roses_game::pads::{GamepadSample, Latch};
    use std::ffi::c_void;
    use std::sync::{Arc, Mutex};

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Guid {
        data1: u32,
        data2: u16,
        data3: u16,
        data4: [u8; 8],
    }
    #[repr(C)]
    struct InterfaceData {
        size: u32,
        class: Guid,
        flags: u32,
        reserved: usize,
    }
    #[repr(C)]
    #[derive(Default)]
    struct Attributes {
        size: u32,
        vendor: u16,
        product: u16,
        version: u16,
    }
    #[repr(C)]
    #[derive(Default)]
    struct Caps {
        usage: u16,
        usage_page: u16,
        input_len: u16,
        output_len: u16,
        feature_len: u16,
        reserved: [u16; 17],
        counts: [u16; 10],
    }

    type Handle = isize;
    const INVALID: Handle = -1;
    const DIGCF_PRESENT: u32 = 0x2;
    const DIGCF_DEVICEINTERFACE: u32 = 0x10;
    const GENERIC_READ: u32 = 0x8000_0000;
    const FILE_SHARE_READ: u32 = 1;
    const FILE_SHARE_WRITE: u32 = 2;
    const OPEN_EXISTING: u32 = 3;
    const HIDP_STATUS_SUCCESS: i32 = 0x0011_0000;

    #[link(name = "hid", kind = "raw-dylib")]
    unsafe extern "system" {
        fn HidD_GetHidGuid(guid: *mut Guid);
        fn HidD_GetAttributes(h: Handle, a: *mut Attributes) -> u8;
        fn HidD_GetPreparsedData(h: Handle, p: *mut isize) -> u8;
        fn HidD_FreePreparsedData(p: isize) -> u8;
        fn HidP_GetCaps(p: isize, caps: *mut Caps) -> i32;
        fn HidD_GetProductString(h: Handle, buf: *mut c_void, len: u32) -> u8;
    }
    #[link(name = "setupapi", kind = "raw-dylib")]
    unsafe extern "system" {
        fn SetupDiGetClassDevsW(class: *const Guid, enumerator: *const u16, parent: isize, flags: u32) -> isize;
        fn SetupDiEnumDeviceInterfaces(set: isize, info: *const c_void, class: *const Guid, index: u32, data: *mut InterfaceData) -> i32;
        fn SetupDiGetDeviceInterfaceDetailW(set: isize, data: *const InterfaceData, detail: *mut c_void, size: u32, required: *mut u32, info: *mut c_void) -> i32;
        fn SetupDiDestroyDeviceInfoList(set: isize) -> i32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateFileW(name: *const u16, access: u32, share: u32, security: *const c_void, disposition: u32, flags: u32, template: Handle) -> Handle;
        fn ReadFile(h: Handle, buf: *mut c_void, len: u32, read: *mut u32, overlapped: *mut c_void) -> i32;
        fn CloseHandle(h: Handle) -> i32;
    }

    /// One open controller: its latest state and the presses since the
    /// last poll, until its reads fail.
    struct Slot {
        path: String,
        latch: Latch,
        alive: bool,
    }

    #[derive(Default)]
    pub struct Sony {
        slots: Arc<Mutex<Vec<Slot>>>,
    }

    impl Sony {
        pub fn start() -> Self {
            if std::env::var("ROSES_NO_HID_PADS").is_ok_and(|v| v == "1") {
                eprintln!("Pads: ROSES_NO_HID_PADS=1: DualSense / DualShock 4 not read");
                return Self::default();
            }
            let slots: Arc<Mutex<Vec<Slot>>> = Arc::default();
            let scan = slots.clone();
            let spawned = std::thread::Builder::new().name("roses-hid-pads".into()).spawn(move || {
                let mut first = true;
                loop {
                    let found = scan_once(&scan);
                    if first && found == 0 {
                        eprintln!("Pads: no DualSense / DualShock 4 found (HID); still looking every 2 s");
                    }
                    first = false;
                    std::thread::sleep(std::time::Duration::from_secs(2));
                }
            });
            if let Err(e) = spawned {
                eprintln!("Pads: the HID controller scan did not start: {e}");
            }
            Self { slots }
        }

        /// Open controllers in connection order.
        pub fn poll(&mut self) -> Vec<GamepadSample> {
            let mut slots = self.slots.lock().unwrap_or_else(|e| e.into_inner());
            slots.retain(|s| s.alive);
            slots.iter_mut().map(|s| s.latch.take()).collect()
        }
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    /// The HID interface paths present now.
    fn hid_paths() -> Vec<String> {
        let mut guid = Guid::default();
        let mut out = Vec::new();
        // SAFETY: documented SetupDi calls with valid buffers; the detail
        // buffer is u32-aligned and as large as the call asked for.
        unsafe {
            HidD_GetHidGuid(&mut guid);
            let set = SetupDiGetClassDevsW(&guid, std::ptr::null(), 0, DIGCF_PRESENT | DIGCF_DEVICEINTERFACE);
            if set == INVALID {
                return out;
            }
            for index in 0.. {
                let mut data = InterfaceData { size: size_of::<InterfaceData>() as u32, class: Guid::default(), flags: 0, reserved: 0 };
                if SetupDiEnumDeviceInterfaces(set, std::ptr::null(), &guid, index, &mut data) == 0 {
                    break;
                }
                let mut required = 0u32;
                SetupDiGetDeviceInterfaceDetailW(set, &data, std::ptr::null_mut(), 0, &mut required, std::ptr::null_mut());
                if required < 8 {
                    continue;
                }
                let mut buf = vec![0u32; (required as usize).div_ceil(4)];
                // cbSize of SP_DEVICE_INTERFACE_DETAIL_DATA_W: 8 on 64-bit, 6 on 32-bit.
                buf[0] = if size_of::<usize>() == 8 { 8 } else { 6 };
                if SetupDiGetDeviceInterfaceDetailW(set, &data, buf.as_mut_ptr().cast(), required, std::ptr::null_mut(), std::ptr::null_mut()) == 0 {
                    continue;
                }
                let chars = std::slice::from_raw_parts(buf.as_ptr().cast::<u16>().add(2), (required as usize - 4) / 2);
                let len = chars.iter().position(|&c| c == 0).unwrap_or(chars.len());
                out.push(String::from_utf16_lossy(&chars[..len]));
            }
            SetupDiDestroyDeviceInfoList(set);
        }
        out
    }

    /// Open the Sony gamepads not open yet; the number open afterwards.
    fn scan_once(slots: &Arc<Mutex<Vec<Slot>>>) -> usize {
        for path in hid_paths() {
            let key = path.to_ascii_lowercase();
            if slots.lock().unwrap_or_else(|e| e.into_inner()).iter().any(|s| s.alive && s.path == key) {
                continue;
            }
            // SAFETY: a NUL-terminated path; the handle is closed by the
            // reader thread or here.
            let probe = unsafe { CreateFileW(wide(&path).as_ptr(), 0, FILE_SHARE_READ | FILE_SHARE_WRITE, std::ptr::null(), OPEN_EXISTING, 0, 0) };
            if probe == INVALID {
                continue;
            }
            let mut attr = Attributes { size: size_of::<Attributes>() as u32, ..Default::default() };
            // SAFETY: valid handle and out-pointer.
            let model = if unsafe { HidD_GetAttributes(probe, &mut attr) } != 0 { sony::model(attr.vendor, attr.product) } else { None };
            let caps = model.and_then(|_| caps(probe)).filter(|c| c.usage_page == 1 && (c.usage == 5 || c.usage == 4));
            let name = product_string(probe);
            // SAFETY: the probe handle is ours.
            unsafe { CloseHandle(probe) };
            let (Some(model), Some(caps)) = (model, caps) else {
                continue;
            };
            // SAFETY: as above, now with read access for ReadFile.
            let h = unsafe { CreateFileW(wide(&path).as_ptr(), GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE, std::ptr::null(), OPEN_EXISTING, 0, 0) };
            let bluetooth = sony::bluetooth(&path, usize::from(caps.input_len));
            let label = format!(
                "{} ({}, {}, {:04X}:{:04X})",
                name.unwrap_or_else(|| model.name().into()),
                model.name(),
                if bluetooth { "Bluetooth" } else { "USB" },
                attr.vendor,
                attr.product
            );
            if h == INVALID {
                eprintln!("Pads: found {label} but could not open it for reading (another program may hold it exclusively)");
                continue;
            }
            eprintln!("Pads: found {label} through HID; input reports of {} bytes", caps.input_len);
            let mut list = slots.lock().unwrap_or_else(|e| e.into_inner());
            list.push(Slot { path: key.clone(), latch: Latch::default(), alive: true });
            drop(list);
            let shared = slots.clone();
            let len = usize::from(caps.input_len).max(64);
            let spawned = std::thread::Builder::new().name("roses-hid-pad".into()).spawn(move || read_loop(h, model, bluetooth, len, &key, &shared, &label));
            if spawned.is_err() {
                // SAFETY: the thread did not take the handle.
                unsafe { CloseHandle(h) };
            }
        }
        slots.lock().unwrap_or_else(|e| e.into_inner()).iter().filter(|s| s.alive).count()
    }

    fn caps(h: Handle) -> Option<Caps> {
        let mut pre = 0isize;
        // SAFETY: valid handle; the preparsed data is freed after use.
        unsafe {
            if HidD_GetPreparsedData(h, &mut pre) == 0 {
                return None;
            }
            let mut caps = Caps::default();
            let ok = HidP_GetCaps(pre, &mut caps) == HIDP_STATUS_SUCCESS;
            HidD_FreePreparsedData(pre);
            ok.then_some(caps)
        }
    }

    fn product_string(h: Handle) -> Option<String> {
        let mut buf = [0u16; 128];
        // SAFETY: the buffer's byte length is passed.
        if unsafe { HidD_GetProductString(h, buf.as_mut_ptr().cast(), (buf.len() * 2) as u32) } == 0 {
            return None;
        }
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        (len > 0).then(|| String::from_utf16_lossy(&buf[..len]))
    }

    fn read_loop(h: Handle, model: Model, bluetooth: bool, len: usize, key: &str, slots: &Mutex<Vec<Slot>>, label: &str) {
        let mut buf = vec![0u8; len];
        let mut reported = std::collections::BTreeSet::new();
        loop {
            let mut read = 0u32;
            // SAFETY: a synchronous handle and a buffer of `len` bytes.
            let ok = unsafe { ReadFile(h, buf.as_mut_ptr().cast(), len as u32, &mut read, std::ptr::null_mut()) } != 0;
            if !ok {
                break;
            }
            let report = &buf[..(read as usize).min(len)];
            let Some(state) = sony::parse(model, bluetooth, report) else {
                continue;
            };
            if reported.insert(report[0]) {
                eprintln!("Pads: {label}: reading input report {:#04x}", report[0]);
            }
            let mut list = slots.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(s) = list.iter_mut().find(|s| s.alive && s.path == key) {
                s.latch.report(state.0, state.1);
            }
        }
        eprintln!("Pads: {label} disconnected");
        if let Some(s) = slots.lock().unwrap_or_else(|e| e.into_inner()).iter_mut().find(|s| s.alive && s.path == key) {
            s.alive = false;
        }
        // SAFETY: the handle is ours.
        unsafe { CloseHandle(h) };
    }
}

#[cfg(not(windows))]
mod hid {
    use roses_game::pads::GamepadSample;
    #[derive(Default)]
    pub struct Sony;
    impl Sony {
        pub fn start() -> Self {
            Sony
        }
        pub fn poll(&mut self) -> Vec<GamepadSample> {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use roses_game::duel_play::{self as dp, PadRepeat};
    use roses_game::pads::Latch;
    use std::collections::BTreeMap;

    /// A scene that reads the keyboard as the duel does (`play_keys`, then
    /// `play_tick`'s held | taps | `pad1_extra` per update).
    #[derive(Resource, Default)]
    struct Scene {
        held: BTreeMap<KeyCode, u16>,
        tapped: u16,
        key_repeats: usize,
    }

    fn scene_keys(mut events: MessageReader<KeyboardInput>, mut sc: ResMut<Scene>) {
        for e in events.read() {
            match e.state {
                ButtonState::Pressed if !e.repeat => {
                    let b = dp::key_pad(&format!("{:?}", e.key_code));
                    if b != 0 {
                        sc.held.insert(e.key_code, b);
                        sc.tapped |= b;
                    }
                }
                ButtonState::Pressed => sc.key_repeats += 1,
                ButtonState::Released => {
                    sc.held.remove(&e.key_code);
                }
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    enum Device {
        Keyboard,
        Gamepad,
    }

    /// Physical button actions `(ms, bit, down)` on one device: keys reach
    /// the next render frame as messages; the gamepad's HID reports reach
    /// the reader thread's latch at once and the frame polls it. Frames
    /// every `frame_ms`, 25 Hz updates (20 ms) after the frames that cover
    /// them, as Bevy's fixed loop runs them. The per-update words.
    fn run(device: Device, actions: &[(f64, u16, bool)], frame_ms: f64, updates: usize) -> Vec<u16> {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::input::InputPlugin));
        app.insert_resource(Pads::without_controllers());
        app.add_systems(PreUpdate, (poll_gamepads.before(InputSystems), collect.after(poll_gamepads)));
        app.init_resource::<Scene>().add_systems(Update, scene_keys);
        let window = app.world_mut().spawn((Window { focused: true, ..default() }, PrimaryWindow)).id();
        let keys = pad1_keys();
        let (mut latch, mut down_bits, mut next, mut t, mut done) = (Latch::default(), 0u16, 0, 0.0, 0usize);
        let mut words = Vec::new();
        while words.len() < updates {
            t += frame_ms;
            while next < actions.len() && actions[next].0 <= t {
                let (_, b, down) = actions[next];
                next += 1;
                match device {
                    Device::Keyboard => {
                        let (_, key_code, logical_key) = keys.iter().find(|(k, _, _)| *k == b).cloned().unwrap();
                        let state = if down { ButtonState::Pressed } else { ButtonState::Released };
                        app.world_mut().write_message(KeyboardInput { key_code, logical_key, state, text: None, repeat: false, window });
                    }
                    Device::Gamepad => {
                        down_bits = if down { down_bits | b } else { down_bits & !b };
                        latch.report(down_bits, pads::STICKS_CENTRED);
                    }
                }
            }
            if device == Device::Gamepad {
                app.world_mut().resource_mut::<Pads>().inject = Some(vec![latch.take()]);
            }
            app.update();
            while (done + 1) as f64 * 20.0 <= t && words.len() < updates {
                done += 1;
                let extra = app.world_mut().resource_mut::<Pads>().pad1_extra();
                let mut sc = app.world_mut().resource_mut::<Scene>();
                let word = sc.held.values().fold(sc.tapped, |a, &b| a | b) | extra;
                sc.tapped = 0;
                words.push(word);
            }
        }
        assert_eq!(app.world().resource::<Scene>().key_repeats, 0, "{device:?}: a key repeat reached the scene");
        words
    }

    /// The duel's presses per update (new presses + the d-pad's repeat).
    fn presses(words: &[u16]) -> Vec<u16> {
        let mut r = PadRepeat::default();
        words.iter().map(|&w| r.frame(w)).collect()
    }

    fn updates_with(words: &[u16], b: u16) -> Vec<usize> {
        words.iter().enumerate().filter(|(_, w)| **w & b != 0).map(|(i, _)| i).collect()
    }

    /// The gamepad and the keyboard give a scene the same word every 25 Hz
    /// update and so the same presses, at 30, 60 and 144 frames a second:
    /// a 5 ms tap counts once, holds repeat only by the original counter.
    #[test]
    fn gamepad_and_keyboard_give_the_same_pad_word_every_update() {
        use bit::*;
        let actions = [
            (30.0, RIGHT, true),
            (105.0, CROSS, true),
            (110.0, CROSS, false),
            (300.0, CIRCLE, true),
            (345.0, CIRCLE, false),
            (500.0, START, true),
            (503.0, START, false),
            (700.0, RIGHT, false),
            (701.0, UP, true),
            (702.0, UP, false),
        ];
        for frame_ms in [1000.0 / 30.0, 1000.0 / 60.0, 1000.0 / 144.0] {
            let keyboard = run(Device::Keyboard, &actions, frame_ms, 50);
            let gamepad = run(Device::Gamepad, &actions, frame_ms, 50);
            if pads::trace_enabled() {
                for (i, (k, g)) in keyboard.iter().zip(&gamepad).enumerate() {
                    eprintln!("ROSES_PAD_TRACE test {frame_ms:.1} ms frames, update {i}: keyboard {k:#06x} gamepad {g:#06x}");
                }
            }
            assert_eq!(keyboard, gamepad, "{frame_ms} ms frames");
            // Taps shorter than a frame count for exactly one update.
            assert_eq!(updates_with(&gamepad, CROSS).len(), 1, "{frame_ms}");
            assert_eq!(updates_with(&gamepad, START).len(), 1, "{frame_ms}");
            assert_eq!(updates_with(&gamepad, UP).len(), 1, "{frame_ms}");
            // Right: held from the update after 30 ms to the one after 700.
            let right = updates_with(&gamepad, RIGHT);
            let (first, last) = (right[0], *right.last().unwrap());
            assert_eq!(right, (first..=last).collect::<Vec<_>>());
            assert!(first <= 2 && (32..=35).contains(&last), "{frame_ms}: {first}..{last}");
            // The duel's presses: the press, then 0x203380 mode 2's first
            // repeat 13 updates later and one every update, plus the
            // update the release is seen — nothing from a key repeat.
            let pressed = updates_with(&presses(&gamepad), RIGHT);
            let want: Vec<usize> = std::iter::once(first).chain(first + 13..=last + 1).collect();
            assert_eq!(pressed, want, "{frame_ms}");
        }
    }

    /// A button still down when a scene starts is not a fresh press for a
    /// keyboard scene from either device (the scene drops the messages
    /// before it existed; the gamepad no longer adds its word directly).
    #[test]
    fn pad1_extra_adds_only_l3_r3_keys() {
        let mut p = Pads::without_controllers();
        let s = pads::GamepadSample { held: bit::START | bit::L3, edges: bit::START | bit::L3, analog: None };
        p.pads.set_gamepads(&[s], &[0]);
        p.pads.key_down("KeyC");
        assert_eq!(p.pad1_extra(), bit::L3);
        assert_eq!(p.take(0), bit::L3 | bit::START);
    }

    /// The four stick bytes reach the scenes in 0x203580's order (the
    /// duel camera reads 1..3, the deck editor 1 and 3, Trade 3); the
    /// keyboard alone has none (0x203500 = 0).
    #[test]
    fn sticks_reach_the_scenes_in_the_originals_order() {
        let mut p = Pads::without_controllers();
        assert_eq!((p.sticks(0), p.analog(0)), (None, None));
        let xinput = roses_game::pads::xinput::sticks(i16::MAX, i16::MIN, i16::MIN, i16::MAX);
        let s = pads::GamepadSample { held: 0, edges: 0, analog: Some(xinput) };
        p.pads.set_gamepads(&[s], &[0]);
        assert_eq!(p.sticks(0), Some([0, 1, 0xFF, 0xFF]));
        assert_eq!((p.analog(0), p.sticks(1)), (Some(0xFF), None));
    }

    /// Every scene's repeat comes from the same original counters (pad
    /// +0x59 = 12, +0x5A & 0xF = 1 [F 0x203180 / 0x203380]): the duel,
    /// Custom Duel and the slot machine (`PadRepeat::frame`), Trade
    /// (`frame_split`), the deck editor (`PadState`) and the map's dialogs
    /// (`MapKeys`, tested there) — first repeat 13 updates after the press,
    /// then every update, one more on the release update. Windows' key
    /// repeat (500 ms, then every 33 ms) would have fired at update 25.
    #[test]
    fn every_scene_repeats_with_the_original_counters() {
        let words: Vec<u16> = (0..40).map(|i| if i < 30 { bit::DOWN } else { 0 }).collect();
        let want: Vec<usize> = std::iter::once(0).chain(13..=30).collect();
        assert_eq!(updates_with(&presses(&words), bit::DOWN), want);
        let mut r = PadRepeat::default();
        let split: Vec<u16> = words.iter().map(|&w| {
            let (edge, rep) = r.frame_split(w);
            edge | rep
        }).collect();
        assert_eq!(updates_with(&split, bit::DOWN), want);
        let mut ed = roses_core::deck_editor::PadState::default();
        let deck: Vec<u16> = words
            .iter()
            .map(|&w| {
                ed.poll(w);
                if ed.repeat(bit::DOWN) { bit::DOWN } else { 0 }
            })
            .collect();
        assert_eq!(updates_with(&deck, bit::DOWN), want);
    }

    #[test]
    fn pad1_keys_are_the_duel_layout() {
        let mut seen = 0u16;
        for (b, code, _) in super::pad1_keys() {
            assert_eq!(super::pads::key_pads(&format!("{code:?}"))[0], b, "{code:?}");
            seen |= b;
        }
        assert_eq!(seen, 0xFFFF);
    }
}
