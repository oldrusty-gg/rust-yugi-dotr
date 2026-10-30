//! Media Foundation (MFPlay) movie playback into a child of an existing
//! top-level window on the calling thread. Shared by frontends that own their
//! own window (the Bevy preview); the native client keeps its own window code.
//! Requires a message pump on the calling thread and a user-imported movie.
use crate::fit_rect;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::ptr::null_mut;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use windows::core::{implement, PCWSTR};
use windows::Win32::Foundation::HWND as MediaWindow;
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MovieStatus {
    Starting,
    Playing,
    Ended,
}
enum Event {
    Ready,
    End,
    Error(i32),
}
#[implement(IMFPMediaPlayerCallback)]
struct Callback {
    sender: SyncSender<Event>,
}
impl IMFPMediaPlayerCallback_Impl for Callback_Impl {
    fn OnMediaPlayerEvent(&self, header: *const MFP_EVENT_HEADER) {
        let Some(header) = (unsafe { header.as_ref() }) else {
            return;
        };
        let event = if header.hrEvent.is_err() || header.eEventType == MFP_EVENT_TYPE_ERROR {
            Some(Event::Error(header.hrEvent.0))
        } else if header.eEventType == MFP_EVENT_TYPE_PLAYBACK_ENDED {
            Some(Event::End)
        } else if header.eEventType == MFP_EVENT_TYPE_PLAY {
            Some(Event::Ready)
        } else {
            None
        };
        if let Some(event) = event {
            let _ = self.sender.try_send(event);
        }
    }
}

fn wide(s: impl AsRef<OsStr>) -> Vec<u16> {
    s.as_ref().encode_wide().chain(Some(0)).collect()
}
/// Same namespace rule as the native client: MF treats `\\?\` as UNC.
fn media_path(path: &std::path::Path) -> Result<Vec<u16>, String> {
    use std::path::{Component, Prefix};
    let Some(Component::Prefix(prefix)) = path.components().next() else {
        return Err("Movie path must be absolute with a Windows prefix.".into());
    };
    let raw: Vec<u16> = path.as_os_str().encode_wide().collect();
    if raw.contains(&0) || raw.len() > 32766 {
        return Err("Movie path contains NUL or is too long.".into());
    }
    let mut converted = match prefix.kind() {
        Prefix::VerbatimDisk(_) => raw[4..].to_vec(),
        Prefix::VerbatimUNC(_, _) => {
            let mut result = vec![b'\\' as u16, b'\\' as u16];
            result.extend_from_slice(&raw[8..]);
            result
        }
        Prefix::Disk(_) | Prefix::UNC(_, _) => raw,
        _ => return Err("Unsupported movie device namespace.".into()),
    };
    converted.push(0);
    Ok(converted)
}

unsafe extern "system" fn video_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // Keyboard focus stays with the frontend window, which owns input.
    if msg == WM_SETFOCUS {
        unsafe {
            windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus(GetParent(hwnd));
        }
        return 0;
    }
    if msg == WM_MOUSEACTIVATE {
        return MA_NOACTIVATE as LRESULT;
    }
    if msg == WM_PAINT {
        let mut paint = PAINTSTRUCT::default();
        unsafe {
            BeginPaint(hwnd, &mut paint);
            let player = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const IMFPMediaPlayer;
            if let Some(player) = player.as_ref() {
                let _ = player.UpdateVideo();
            }
            EndPaint(hwnd, &paint);
        }
        return 0;
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// The calling thread's visible top-level window (the frontend's window).
fn thread_window() -> Option<HWND> {
    unsafe extern "system" fn visit(hwnd: HWND, found: LPARAM) -> windows_sys::core::BOOL {
        unsafe {
            if IsWindowVisible(hwnd) != 0 && GetParent(hwnd).is_null() {
                *(found as *mut HWND) = hwnd;
                return 0;
            }
        }
        1
    }
    let mut found: HWND = null_mut();
    unsafe {
        EnumThreadWindows(
            GetCurrentThreadId(),
            Some(visit),
            &mut found as *mut HWND as LPARAM,
        );
    }
    (!found.is_null()).then_some(found)
}

pub struct MoviePlayer {
    parent: HWND,
    video: HWND,
    // Boxed so the child window's GWLP_USERDATA pointer stays valid.
    player: Box<IMFPMediaPlayer>,
    receiver: Receiver<Event>,
    status: MovieStatus,
    aspect: (i32, i32),
}

impl MoviePlayer {
    /// Start `path` inside the calling thread's top-level window, fitted to
    /// `aspect` (width, height). The caller keeps presenting its own frame
    /// underneath; the child window covers it until dropped.
    pub fn start(path: &std::path::Path, aspect: (u32, u32)) -> Result<Self, String> {
        let parent = thread_window().ok_or("No top-level window on this thread for the movie.")?;
        unsafe {
            // S_FALSE (already initialized) is fine; a different model fails.
            CoInitializeEx(None, COINIT_APARTMENTTHREADED)
                .ok()
                .or_else(|e| if e.code().0 == 1 { Ok(()) } else { Err(e) })
                .map_err(|e| format!("COM initialization for movie failed: {e}"))?;
        }
        let class = wide("RosesSharedMovieSurface1");
        let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
        unsafe {
            let wc = WNDCLASSW {
                lpfnWndProc: Some(video_proc),
                hInstance: instance,
                lpszClassName: class.as_ptr(),
                hbrBackground: GetStockObject(BLACK_BRUSH) as HBRUSH,
                ..Default::default()
            };
            // Re-registration after a previous movie reports ALREADY_EXISTS.
            RegisterClassW(&wc);
        }
        let video = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                wide("").as_ptr(),
                WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
                0,
                0,
                1,
                1,
                parent,
                null_mut(),
                instance,
                std::ptr::null(),
            )
        };
        if video.is_null() {
            return Err("Could not create the movie surface.".into());
        }
        let (sender, receiver) = sync_channel(32);
        let callback: IMFPMediaPlayerCallback = Callback { sender }.into();
        let wide_path = media_path(path)?;
        let mut player = None;
        let created = unsafe {
            MFPCreateMediaPlayer(
                PCWSTR(wide_path.as_ptr()),
                true,
                MFP_OPTION_NONE,
                &callback,
                Some(MediaWindow(video)),
                Some(&mut player),
            )
        };
        let Some(player) = created.ok().and(player) else {
            unsafe {
                DestroyWindow(video);
            }
            return Err("Native movie could not open.".into());
        };
        let player = Box::new(player);
        unsafe {
            SetWindowLongPtrW(
                video,
                GWLP_USERDATA,
                &*player as *const IMFPMediaPlayer as isize,
            );
        }
        let mut this = Self {
            parent,
            video,
            player,
            receiver,
            status: MovieStatus::Starting,
            aspect: (aspect.0 as i32, aspect.1 as i32),
        };
        this.fit();
        Ok(this)
    }

    /// Keep the surface fitted to the parent's client area (call per frame).
    pub fn fit(&mut self) {
        unsafe {
            let mut rect = RECT::default();
            GetClientRect(self.parent, &mut rect);
            let [x, y, w, h] = fit_rect(
                rect.right,
                rect.bottom,
                self.aspect.0 as u32,
                self.aspect.1 as u32,
            );
            let mut current = RECT::default();
            GetWindowRect(self.video, &mut current);
            if current.right - current.left != w || current.bottom - current.top != h {
                MoveWindow(self.video, x, y, w, h, 1);
                InvalidateRect(self.video, std::ptr::null(), 0);
            } else {
                SetWindowPos(
                    self.video,
                    HWND_TOP,
                    x,
                    y,
                    w,
                    h,
                    SWP_NOACTIVATE | SWP_NOSIZE,
                );
            }
        }
    }

    pub fn poll(&mut self) -> Result<MovieStatus, String> {
        while let Ok(event) = self.receiver.try_recv() {
            match event {
                Event::Ready => {
                    unsafe {
                        self.player
                            .SetAspectRatioMode(MFVideoARMode_None.0 as u32)
                            .map_err(|e| format!("Movie aspect could not be applied: {e}"))?;
                        InvalidateRect(self.video, std::ptr::null(), 0);
                    }
                    self.status = MovieStatus::Playing;
                }
                Event::End => self.status = MovieStatus::Ended,
                Event::Error(code) => {
                    return Err(format!(
                        "Native movie playback failed (HRESULT 0x{:08X}).",
                        code as u32
                    ));
                }
            }
        }
        Ok(self.status)
    }
}

impl Drop for MoviePlayer {
    fn drop(&mut self) {
        unsafe {
            SetWindowLongPtrW(self.video, GWLP_USERDATA, 0);
            let _ = self.player.Shutdown();
            DestroyWindow(self.video);
        }
    }
}
