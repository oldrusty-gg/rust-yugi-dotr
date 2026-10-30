//! Win32 presentation and native Media Foundation playback. No emulator process.
use crate::diagnostics::record;
use roses_client::wave_out::WavePlayer;
use roses_client::{fit_rect, music_continues, Button, Loaded, Model, Music, Outcome, Stage};
use std::cell::RefCell;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::ptr::{null, null_mut};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::time::Instant;
use windows::core::{implement, PCWSTR};
use windows::Win32::Foundation::HWND as MediaWindow;
use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_FILENAME, SND_LOOP, SND_NODEFAULT};
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::Input::XboxController::{XInputGetState, XINPUT_STATE};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

fn wide(s: impl AsRef<OsStr>) -> Vec<u16> {
    s.as_ref().encode_wide().chain(Some(0)).collect()
}
/// Media Foundation's URL resolver interprets a verbatim `\\?\` path as UNC.
/// Change only the namespace spelling; validation and locked handles continue
/// to use the original canonical path. Never strip arbitrary device prefixes.
fn media_path(path: &std::path::Path) -> Result<Vec<u16>, String> {
    use std::path::{Component, Prefix};
    if !path.is_absolute() {
        return Err("Media path must be absolute.".into());
    }
    let Some(Component::Prefix(prefix)) = path.components().next() else {
        return Err("Media path has no Windows prefix.".into());
    };
    let raw: Vec<u16> = path.as_os_str().encode_wide().collect();
    if raw.contains(&0) || raw.len() > 32766 {
        return Err("Media path contains NUL or is too long.".into());
    }
    let mut converted = match prefix.kind() {
        Prefix::VerbatimDisk(_) => raw[4..].to_vec(),
        Prefix::VerbatimUNC(_, _) => {
            let mut result = vec![b'\\' as u16, b'\\' as u16];
            result.extend_from_slice(&raw[8..]);
            result
        }
        Prefix::Disk(_) | Prefix::UNC(_, _) => raw,
        _ => return Err("Unsupported media device namespace.".into()),
    };
    converted.push(0);
    Ok(converted)
}
pub fn show_error(message: &str) {
    unsafe {
        MessageBoxW(
            null_mut(),
            wide(message).as_ptr(),
            wide("Roses native opening").as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}
enum MediaEvent {
    Ready(u64),
    End(u64),
    Error(u64, i32),
}
#[implement(IMFPMediaPlayerCallback)]
struct PlayerCallback {
    sender: SyncSender<MediaEvent>,
    generation: u64,
}
impl IMFPMediaPlayerCallback_Impl for PlayerCallback_Impl {
    fn OnMediaPlayerEvent(&self, header: *const MFP_EVENT_HEADER) {
        // MF owns this event only for the callback; retain no COM/event pointers.
        let Some(header) = (unsafe { header.as_ref() }) else {
            return;
        };
        record(&format!(
            "Media callback generation={} type={} HRESULT=0x{:08X} state={}",
            self.generation, header.eEventType.0, header.hrEvent.0 as u32, header.eState.0
        ));
        let event = if header.hrEvent.is_err() || header.eEventType == MFP_EVENT_TYPE_ERROR {
            Some(MediaEvent::Error(self.generation, header.hrEvent.0))
        } else if header.eEventType == MFP_EVENT_TYPE_PLAYBACK_ENDED {
            Some(MediaEvent::End(self.generation))
        } else if header.eEventType == MFP_EVENT_TYPE_PLAY {
            Some(MediaEvent::Ready(self.generation))
        } else {
            None
        };
        if let Some(event) = event {
            let _ = self.sender.try_send(event);
        }
    }
}
struct App {
    loaded: Loaded,
    model: Model,
    frame: Vec<u8>,
    video: HWND,
    player: Option<IMFPMediaPlayer>,
    wave_player: Option<WavePlayer>,
    active_music: Option<Music>,
    generation: u64,
    sender: SyncSender<MediaEvent>,
    receiver: Receiver<MediaEvent>,
    started: Instant,
    pads: [u16; 4],
    fullscreen: bool,
    placement: WINDOWPLACEMENT,
    failure: Option<String>,
}
type Shared = RefCell<App>;
struct Com;
impl Drop for Com {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

pub fn run(loaded: Loaded, fullscreen: bool) -> Result<(), String> {
    record("Initialize native COM/window");
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .map_err(|e| e.to_string())?;
    }
    let _com = Com;
    let model = Model::new(loaded.manifest.clone())?;
    let frame = loaded.render(&model)?;
    let (sender, receiver) = sync_channel(32);
    let shared = Box::new(RefCell::new(App {
        loaded,
        model,
        frame,
        video: null_mut(),
        player: None,
        wave_player: None,
        active_music: None,
        generation: 0,
        sender,
        receiver,
        started: Instant::now(),
        pads: [0; 4],
        fullscreen: false,
        placement: WINDOWPLACEMENT {
            length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
            ..Default::default()
        },
        failure: None,
    }));
    unsafe {
        let instance = GetModuleHandleW(null());
        let class = wide("RosesNativeOpeningWindow1");
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            lpszClassName: class.as_ptr(),
            ..Default::default()
        };
        if RegisterClassW(&wc) == 0 {
            return Err("Could not register opening window.".into());
        }
        let pointer = (&*shared as *const Shared).cast_mut().cast();
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            wide("Roses — native opening slice").as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            976,
            759,
            null_mut(),
            null_mut(),
            instance,
            pointer,
        );
        if hwnd.is_null() {
            UnregisterClassW(class.as_ptr(), instance);
            return Err("Could not create opening window.".into());
        }
        record(&format!("Main window created {hwnd:?}"));
        let video = CreateWindowExW(
            0,
            class.as_ptr(),
            wide("").as_ptr(),
            WS_CHILD | WS_CLIPSIBLINGS,
            0,
            0,
            1,
            1,
            hwnd,
            null_mut(),
            instance,
            pointer,
        );
        if video.is_null() {
            DestroyWindow(hwnd);
            UnregisterClassW(class.as_ptr(), instance);
            return Err("Could not create video surface.".into());
        }
        shared.borrow_mut().video = video;
        record(&format!("Video surface created {video:?}"));
        ShowWindow(hwnd, SW_SHOW);
        if fullscreen {
            toggle_fullscreen(hwnd, &shared);
        }
        resize_video(hwnd, &shared);
        if let Err(error) = enter_stage(hwnd, &shared) {
            shared.borrow_mut().failure = Some(error);
            close(hwnd, &shared);
        } else if SetTimer(hwnd, 1, 16, None) == 0 {
            shared.borrow_mut().failure = Some("Could not create input timer.".into());
            close(hwnd, &shared);
        }
        let mut msg = MSG::default();
        loop {
            let status = GetMessageW(&mut msg, null_mut(), 0, 0);
            if status <= 0 {
                record(&format!(
                    "Message loop exit status={status} quit-code={}",
                    msg.wParam
                ));
                if status < 0 {
                    shared.borrow_mut().failure = Some("Window message loop failed.".into());
                }
                if status == 0 && msg.wParam != 0 && shared.borrow().failure.is_none() {
                    shared.borrow_mut().failure = Some(
                        "Native window handler failed; see roses-client.log beside the executable."
                            .into(),
                    );
                }
                break;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        // Even an unexpected WM_QUIT/GetMessage failure must not leave a live
        // window with GWLP_USERDATA pointing at the soon-to-be-dropped Box.
        if IsWindow(hwnd) != 0 {
            close(hwnd, &shared);
        } else {
            stop_media(&shared, false);
        }
        UnregisterClassW(class.as_ptr(), instance);
    }
    let failure = shared.borrow_mut().failure.take();
    failure.map_or(Ok(()), Err)
}
unsafe fn stop_media(shared: &Shared, preserve_music: bool) {
    let (player, wave_player) = {
        let mut app = shared.borrow_mut();
        app.generation += 1;
        let wave_player = if preserve_music {
            None
        } else {
            app.active_music = None;
            app.wave_player.take()
        };
        (app.player.take(), wave_player)
    };
    if let Some(player) = player {
        record("Shutting down media player");
        let _ = player.Shutdown();
    }
    drop(wave_player);
    if !preserve_music {
        let _ = PlaySoundW(PCWSTR::null(), None, SND_ASYNC);
    }
}
unsafe fn close(hwnd: HWND, shared: &Shared) {
    record(&format!("Closing native window {hwnd:?}"));
    KillTimer(hwnd, 1);
    stop_media(shared, false);
    DestroyWindow(hwnd);
}
unsafe fn enter_stage(hwnd: HWND, shared: &Shared) -> Result<(), String> {
    let preserve_music = {
        let app = shared.borrow();
        music_continues(app.active_music.as_ref(), app.model.stage().music())
    };
    stop_media(shared, preserve_music);
    let (stage, video, generation, sender) = {
        let mut app = shared.borrow_mut();
        app.started = Instant::now();
        app.frame = app.loaded.render(&app.model)?;
        (
            app.model.stage().clone(),
            app.video,
            app.generation,
            app.sender.clone(),
        )
    };
    record(&format!(
        "Entering stage {} generation={generation}",
        stage.id()
    ));
    match stage {
        Stage::Movie { asset, .. } => {
            ShowWindow(video, SW_SHOW);
            let path = {
                let app = shared.borrow();
                media_path(&app.loaded.assets[&asset].path)?
            };
            let callback: IMFPMediaPlayerCallback = PlayerCallback { sender, generation }.into();
            let mut player = None;
            record("MFPCreateMediaPlayer begin");
            MFPCreateMediaPlayer(
                PCWSTR(path.as_ptr()),
                true,
                MFP_OPTION_NONE,
                &callback,
                Some(MediaWindow(video)),
                Some(&mut player),
            )
            .map_err(|e| format!("Native video could not open: {e}"))?;
            record("MFPCreateMediaPlayer returned successfully");
            shared.borrow_mut().player = player;
            // Creation with a URL is asynchronous. Video renderer configuration
            // is invalid until PLAY reports that the player has started.
            record("Media player retained; awaiting PLAY event before aspect configuration");
        }
        _ => {
            ShowWindow(video, SW_HIDE);
            if let Some(music) = stage.music().filter(|_| !preserve_music) {
                if let Some(start) = music.loop_start_frame {
                    let pcm = shared.borrow().loaded.assets[&music.asset]
                        .pcm16
                        .clone()
                        .ok_or("Loop audio was not validated.")?;
                    let player = WavePlayer::start(pcm, start, record)?;
                    shared.borrow_mut().wave_player = Some(player);
                } else {
                    let path = {
                        let app = shared.borrow();
                        media_path(&app.loaded.assets[&music.asset].path)?
                    };
                    let mut flags = SND_ASYNC | SND_FILENAME | SND_NODEFAULT;
                    if music.looped {
                        flags |= SND_LOOP;
                    }
                    if !PlaySoundW(PCWSTR(path.as_ptr()), None, flags).as_bool() {
                        return Err("Native WAV playback failed.".into());
                    }
                }
                shared.borrow_mut().active_music = Some(music.clone());
            }
        }
    }
    InvalidateRect(hwnd, null(), 1);
    Ok(())
}

unsafe fn outcome(hwnd: HWND, shared: &Shared, result: Outcome) {
    let update = match result {
        Outcome::StageChanged => enter_stage(hwnd, shared),
        Outcome::Repaint => {
            let mut app = shared.borrow_mut();
            match app.loaded.render(&app.model) {
                Ok(frame) => {
                    app.frame = frame;
                    InvalidateRect(hwnd, null(), 0);
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }
        Outcome::Unavailable(label) => {
            MessageBoxW(hwnd,wide(format!("The native opening slice does not yet implement this branch: {label}\n\nThe original game assets are shown here; story and duel playback are still under development.")).as_ptr(),wide("Roses implementation status").as_ptr(),MB_OK|MB_ICONINFORMATION);
            Ok(())
        }
        Outcome::Unsupported(message) => {
            MessageBoxW(
                hwnd,
                wide(message).as_ptr(),
                wide("Roses implementation status").as_ptr(),
                MB_OK | MB_ICONINFORMATION,
            );
            Ok(())
        }
        Outcome::Tutorial { resume } => {
            MessageBoxW(
                hwnd,
                wide(format!("The scripted tutorial (three practice lessons on the duel board) is played by the Bevy game (roses-bevy-scene --game), not by this native opening slice; the story would resume at stage {resume}.")).as_ptr(),
                wide("Roses implementation status").as_ptr(),
                MB_OK | MB_ICONINFORMATION,
            );
            Ok(())
        }
        Outcome::MapEvent(event) => {
            MessageBoxW(
                hwnd,
                wide(format!("The campaign map continues at map event {event} in the Bevy game (roses-bevy-scene --game), not in this native opening slice.")).as_ptr(),
                wide("Roses implementation status").as_ptr(),
                MB_OK | MB_ICONINFORMATION,
            );
            Ok(())
        }
        Outcome::None => Ok(()),
    };
    if let Err(error) = update {
        record(&format!("Stage update failed: {error}"));
        shared.borrow_mut().failure = Some(error);
        close(hwnd, shared);
    }
}
unsafe fn input(hwnd: HWND, shared: &Shared, button: Button) {
    record(&format!("Input {button:?}"));
    let result = shared.borrow_mut().model.input(button);
    {
        let app = shared.borrow();
        if matches!(app.model.stage(), Stage::Story { .. }) {
            record(&format!(
                "Story {} page_index={}",
                app.model.stage().id(),
                app.model.page_index()
            ));
        }
        if let Some(editor) = app.model.name_entry() {
            record(&format!(
                "Name entry page={} row={} column={} length={} committed={}",
                editor.page(),
                editor.row(),
                editor.column(),
                editor.glyphs().len(),
                editor.committed()
            ));
        }
    }
    outcome(hwnd, shared, result);
}
unsafe fn tick(hwnd: HWND, shared: &Shared) {
    loop {
        let event = shared.borrow().receiver.try_recv().ok();
        let Some(event) = event else {
            break;
        };
        let active_generation = shared.borrow().generation;
        match event {
            MediaEvent::Ready(generation) if generation == active_generation => {
                let player = shared.borrow().player.clone();
                let result=match player {
                    Some(player)=>player.SetAspectRatioMode(MFVideoARMode_None.0 as u32).map_err(|error|format!("Could not apply the observed video aspect after playback started: {error}")),
                    None=>Err("Playback-ready event has no retained media player.".into()),
                };
                if let Err(error) = result {
                    record(&error);
                    shared.borrow_mut().failure = Some(error);
                    close(hwnd, shared);
                    return;
                }
                record("Playback ready; observed display aspect configured");
                let video = shared.borrow().video;
                InvalidateRect(video, null(), 0);
            }
            MediaEvent::End(generation) if generation == active_generation => {
                let result = shared.borrow_mut().model.video_ended();
                outcome(hwnd, shared, result);
            }
            MediaEvent::Error(generation, code) if generation == active_generation => {
                shared.borrow_mut().failure = Some(format!(
                    "Native media playback failed (HRESULT 0x{:08X}).",
                    code as u32
                ));
                close(hwnd, shared);
                return;
            }
            _ => {}
        }
        if IsWindow(hwnd) == 0 {
            return;
        }
    }
    let result = {
        let mut app = shared.borrow_mut();
        let elapsed = app.started.elapsed().as_millis();
        app.model.elapsed(elapsed)
    };
    outcome(hwnd, shared, result);
    if IsWindow(hwnd) == 0 {
        return;
    }
    let focused = GetForegroundWindow() == hwnd;
    for index in 0..4 {
        let mut state = XINPUT_STATE::default();
        let mut held = if XInputGetState(index, &mut state) == 0 {
            state.Gamepad.wButtons.0
        } else {
            0
        };
        if state.Gamepad.sThumbLY > 16384 {
            held |= 1;
        }
        if state.Gamepad.sThumbLY < -16384 {
            held |= 2;
        }
        if state.Gamepad.sThumbLX > 16384 {
            held |= 8;
        }
        if state.Gamepad.sThumbLX < -16384 {
            held |= 4;
        }
        let fresh = {
            let mut app = shared.borrow_mut();
            let old = app.pads[index as usize];
            app.pads[index as usize] = held;
            held & !old
        };
        if focused {
            for (mask, button) in [
                (1, Button::Up),
                (2, Button::Down),
                (4, Button::Left),
                (8, Button::Right),
                (4096, Button::Cross),
                (8192, Button::Circle),
                (16384, Button::Square),
                (256, Button::L1),
                (512, Button::R1),
                (16, Button::Start),
            ] {
                if fresh & mask != 0 {
                    input(hwnd, shared, button);
                    if IsWindow(hwnd) == 0 {
                        return;
                    }
                }
            }
        }
    }
}
unsafe fn resize_video(hwnd: HWND, shared: &Shared) {
    let video = shared.borrow().video;
    if video.is_null() {
        return;
    }
    let mut rect = RECT::default();
    GetClientRect(hwnd, &mut rect);
    let canvas = shared.borrow().loaded.manifest.canvas;
    let [x, y, width, height] = fit_rect(
        rect.right,
        rect.bottom,
        canvas.aspect_width,
        canvas.aspect_height,
    );
    MoveWindow(video, x, y, width, height, 1);
    // WM_PAINT brackets UpdateVideo with BeginPaint/EndPaint as MFPlay requires.
    InvalidateRect(video, null(), 0);
}
unsafe fn toggle_fullscreen(hwnd: HWND, shared: &Shared) {
    let fullscreen = shared.borrow().fullscreen;
    if !fullscreen {
        let mut placement = WINDOWPLACEMENT {
            length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
            ..Default::default()
        };
        if GetWindowPlacement(hwnd, &mut placement) == 0 {
            return;
        }
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(monitor, &mut info) == 0 {
            return;
        }
        {
            let mut app = shared.borrow_mut();
            app.fullscreen = true;
            app.placement = placement;
        }
        SetWindowLongPtrW(
            hwnd,
            GWL_STYLE,
            (WS_POPUP | WS_VISIBLE | WS_CLIPCHILDREN) as isize,
        );
        let r = info.rcMonitor;
        SetWindowPos(
            hwnd,
            HWND_TOP,
            r.left,
            r.top,
            r.right - r.left,
            r.bottom - r.top,
            SWP_FRAMECHANGED | SWP_NOOWNERZORDER,
        );
    } else {
        let placement = {
            let mut app = shared.borrow_mut();
            app.fullscreen = false;
            app.placement
        };
        SetWindowLongPtrW(
            hwnd,
            GWL_STYLE,
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE | WS_CLIPCHILDREN) as isize,
        );
        SetWindowPlacement(hwnd, &placement);
        SetWindowPos(
            hwnd,
            null_mut(),
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOOWNERZORDER,
        );
    }
}
unsafe fn paint(hwnd: HWND, shared: &Shared) {
    let mut ps = PAINTSTRUCT::default();
    let dc = BeginPaint(hwnd, &mut ps);
    let mut rect = RECT::default();
    GetClientRect(hwnd, &mut rect);
    FillRect(dc, &rect, GetStockObject(BLACK_BRUSH) as HBRUSH);
    let Ok(app) = shared.try_borrow() else {
        EndPaint(hwnd, &ps);
        return;
    };
    if hwnd == app.video {
        let player = app.player.clone();
        drop(app);
        if let Some(player) = player {
            let _ = player.UpdateVideo();
        }
    } else if !matches!(app.model.stage(), Stage::Movie { .. }) {
        let c = app.loaded.manifest.canvas;
        let [x, y, w, h] = fit_rect(rect.right, rect.bottom, c.aspect_width, c.aspect_height);
        let bitmap = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: c.width as i32,
                biHeight: -(c.height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..Default::default()
            },
            ..Default::default()
        };
        SetStretchBltMode(dc, COLORONCOLOR);
        StretchDIBits(
            dc,
            x,
            y,
            w,
            h,
            0,
            0,
            c.width as i32,
            c.height as i32,
            app.frame.as_ptr().cast(),
            &bitmap,
            DIB_RGB_COLORS,
            SRCCOPY,
        );
        if let Some(text) = app.model.explicit_status() {
            draw_status(dc, &text, [x, y, w, h]);
        }
    }
    EndPaint(hwnd, &ps);
}
/// Untraced parts stay explicit (stand-in messages, deck-select candidates and
/// selection): the shared status text is drawn in the frame's top band, which
/// this client leaves black (no 3D scene, card stack or map), so it is also
/// visible in fullscreen. Canvas rows 0..116 hold no deck-select layer.
unsafe fn draw_status(dc: HDC, text: &str, [x, y, w, h]: [i32; 4]) {
    let font = CreateFontW(
        -(h / 32).max(12),
        0,
        0,
        0,
        FW_NORMAL as i32,
        0,
        0,
        0,
        u32::from(DEFAULT_CHARSET),
        0,
        0,
        0,
        u32::from(FF_SWISS),
        wide("Segoe UI").as_ptr(),
    );
    let previous = SelectObject(dc, font);
    SetBkMode(dc, TRANSPARENT as i32);
    SetTextColor(dc, 0x00FF_FFFF);
    let lines = text
        .replacen("] ", "]\n", 1)
        .replace(" |", "\n")
        .replace(" · ", "\n");
    let wide_text = wide(lines);
    let margin = (w / 40).max(4);
    let mut rect = RECT {
        left: x + margin,
        top: y + margin,
        right: x + w - margin,
        bottom: y + h * 116 / 448,
    };
    DrawTextW(
        dc,
        wide_text.as_ptr(),
        -1,
        &mut rect,
        DT_WORDBREAK | DT_NOPREFIX,
    );
    SelectObject(dc, previous);
    DeleteObject(font);
}
unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // A Rust panic must not unwind across a Win32 ABI boundary. The panic hook
    // preserves its backtrace; normal message-loop cleanup owns the window.
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        window_proc_inner(hwnd, message, wparam, lparam)
    })) {
        Ok(result) => result,
        Err(_) => {
            record(&format!("Window handler panic, message=0x{message:04X}"));
            PostQuitMessage(1);
            0
        }
    }
}
unsafe fn window_proc_inner(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if message == WM_NCCREATE {
        let create = &*(lparam as *const CREATESTRUCTW);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
    }
    let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Shared;
    if pointer.is_null() {
        return DefWindowProcW(hwnd, message, wparam, lparam);
    }
    let shared = &*pointer;
    // Native rendering/media APIs can synchronously reenter a window procedure.
    // Never create aliased mutable App references across such calls.
    let Ok(app) = shared.try_borrow() else {
        return DefWindowProcW(hwnd, message, wparam, lparam);
    };
    let child = hwnd == app.video;
    drop(app);
    match message {
        WM_PAINT => {
            paint(hwnd, shared);
            0
        }
        WM_ERASEBKGND => 1,
        WM_SIZE if !child => {
            resize_video(hwnd, shared);
            0
        }
        WM_TIMER if !child => {
            tick(hwnd, shared);
            0
        }
        WM_KEYDOWN if !child && lparam & (1 << 30) == 0 => {
            if wparam == 0x7A {
                toggle_fullscreen(hwnd, shared);
            } else {
                let button = match wparam {
                    0x26 => Some(Button::Up),
                    0x28 => Some(Button::Down),
                    0x25 => Some(Button::Left),
                    0x27 => Some(Button::Right),
                    0x58 => Some(Button::Cross),
                    0x41 => Some(Button::Square),
                    0x51 => Some(Button::L1),
                    0x45 => Some(Button::R1),
                    0x5A | 0x1B => Some(Button::Circle),
                    0x53 => Some(Button::Start),
                    0x0D => shared.borrow().model.stage().primary_button(),
                    _ => None,
                };
                if let Some(button) = button {
                    input(hwnd, shared, button);
                }
            }
            0
        }
        WM_CLOSE if !child => {
            record("Received WM_CLOSE");
            close(hwnd, shared);
            0
        }
        WM_DESTROY if !child => {
            record("Received main WM_DESTROY");
            PostQuitMessage(0);
            0
        }
        WM_NCDESTROY => {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            DefWindowProcW(hwnd, message, wparam, lparam)
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

#[cfg(test)]
mod tests {
    use super::media_path;
    use std::path::Path;
    fn transformed(path: &str) -> String {
        let value = media_path(Path::new(path)).unwrap();
        assert_eq!(value.last(), Some(&0));
        String::from_utf16(&value[..value.len() - 1]).unwrap()
    }
    #[test]
    fn media_paths_remove_only_recognized_verbatim_prefixes() {
        assert_eq!(
            transformed(r"\\?\C:\Games\Rose Ω\opening.mp4"),
            r"C:\Games\Rose Ω\opening.mp4"
        );
        assert_eq!(
            transformed(r"\\?\UNC\server\share\opening.mp4"),
            r"\\server\share\opening.mp4"
        );
        assert_eq!(
            transformed(r"C:\Games\opening.mp4"),
            r"C:\Games\opening.mp4"
        );
        assert_eq!(
            transformed(r"\\server\share\opening.mp4"),
            r"\\server\share\opening.mp4"
        );
    }
    #[test]
    fn media_paths_reject_relative_and_device_namespaces() {
        for value in [
            r"opening.mp4",
            r"C:opening.mp4",
            r"\\.\pipe\movie",
            r"\\?\Volume{0123}\movie",
        ] {
            assert!(media_path(Path::new(value)).is_err(), "{value}");
        }
        assert!(media_path(Path::new("C:\\bad\0name.mp4")).is_err());
    }
}
