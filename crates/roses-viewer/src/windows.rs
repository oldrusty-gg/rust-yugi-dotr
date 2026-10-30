//! Original GDI placeholders. There is no texture/card-art ID inference here.
use crate::app::{Input, Loaded};
use roses_viewer::{BoardLayout, GridPos, Occupancy, ViewOrigin};
use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;
use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::Dialogs::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    SetFocus, VK_DOWN, VK_LEFT, VK_RIGHT, VK_UP,
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const DEMO: usize = 101;
const CAPTURE: usize = 102;
const SNAPSHOT: usize = 103;
const LIBRARY: usize = 104;
const BACKGROUND: u32 = 0x00F4F1EC;
const TEXT: u32 = 0x003C3025;

struct App {
    loaded: Loaded,
    selected: GridPos,
    layout: BoardLayout,
    normal_font: HFONT,
    small_font: HFONT,
    title_font: HFONT,
    buttons: [HWND; 4],
    source: HWND,
    details: HWND,
    notice: String,
}

fn wide(value: impl AsRef<OsStr>) -> Vec<u16> {
    value.as_ref().encode_wide().chain(Some(0)).collect()
}
pub fn show_error(error: &str) {
    unsafe {
        MessageBoxW(
            null_mut(),
            wide(error).as_ptr(),
            wide("Roses board viewer").as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}
unsafe fn font(height: i32, weight: i32) -> HFONT {
    CreateFontW(
        -height,
        0,
        0,
        0,
        weight,
        0,
        0,
        0,
        1,
        0,
        0,
        5,
        0,
        wide("Segoe UI").as_ptr(),
    )
}
pub fn run(loaded: Loaded) -> Result<(), String> {
    unsafe {
        let mut app = Box::new(App {
            loaded,
            selected: GridPos { column: 0, row: 0 },
            layout: BoardLayout {
                left: 40,
                top: 190,
                tile: 70,
            },
            normal_font: font(17, 400),
            small_font: font(14, 400),
            title_font: font(25, 600),
            buttons: [null_mut(); 4],
            source: null_mut(),
            details: null_mut(),
            notice: String::new(),
        });
        if [app.normal_font, app.small_font, app.title_font]
            .iter()
            .any(|font| font.is_null())
        {
            for font in [app.normal_font, app.small_font, app.title_font] {
                if !font.is_null() {
                    DeleteObject(font);
                }
            }
            return Err("Could not create viewer fonts.".into());
        }
        let result = message_loop(&mut app);
        for font in [app.normal_font, app.small_font, app.title_font] {
            DeleteObject(font);
        }
        result
    }
}

unsafe fn message_loop(app: &mut App) -> Result<(), String> {
    let instance = GetModuleHandleW(null());
    let class_name = wide("RosesBoardViewer");
    let class = WNDCLASSW {
        lpfnWndProc: Some(window_proc),
        hInstance: instance,
        hCursor: LoadCursorW(null_mut(), IDC_ARROW),
        lpszClassName: class_name.as_ptr(),
        ..Default::default()
    };
    if RegisterClassW(&class) == 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    // App is boxed by run() and remains valid until this complete message loop exits.
    let mut work = RECT {
        left: 0,
        top: 0,
        right: 1024,
        bottom: 768,
    };
    SystemParametersInfoW(SPI_GETWORKAREA, 0, (&mut work as *mut RECT).cast(), 0);
    let width = 1060.min((work.right - work.left - 36).max(640));
    let height = 790.min((work.bottom - work.top - 36).max(480));
    let hwnd = CreateWindowExW(
        WS_EX_CONTROLPARENT,
        class_name.as_ptr(),
        wide("Roses — Board Snapshot Viewer").as_ptr(),
        WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
        work.left + 18,
        work.top + 18,
        width,
        height,
        null_mut(),
        null_mut(),
        instance,
        (app as *mut App).cast(),
    );
    if hwnd.is_null() {
        return Err(format!(
            "Could not create viewer window: {}",
            std::io::Error::last_os_error()
        ));
    }
    ShowWindow(hwnd, SW_SHOW);
    let mut message = MSG::default();
    loop {
        let status = GetMessageW(&mut message, null_mut(), 0, 0);
        if status == 0 {
            break;
        }
        if status == -1 {
            DestroyWindow(hwnd);
            return Err(std::io::Error::last_os_error().to_string());
        }
        // The board itself receives arrows; controls retain normal Tab navigation.
        if message.hwnd == hwnd || IsDialogMessageW(hwnd, &message) == 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    Ok(())
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        SetWindowLongPtrW(
            hwnd,
            GWLP_USERDATA,
            (*(lparam as *const CREATESTRUCTW)).lpCreateParams as isize,
        );
        return DefWindowProcW(hwnd, message, wparam, lparam);
    }
    let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App;
    if state.is_null() {
        return DefWindowProcW(hwnd, message, wparam, lparam);
    }
    match message {
        WM_CREATE => {
            if create_controls(hwnd, &mut *state).is_err() {
                return -1;
            }
            resize(hwnd, &mut *state);
            update_details(&mut *state);
            0
        }
        WM_SIZE => {
            resize(hwnd, &mut *state);
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_GETMINMAXINFO => {
            let mut work = RECT {
                left: 0,
                top: 0,
                right: 1024,
                bottom: 768,
            };
            SystemParametersInfoW(SPI_GETWORKAREA, 0, (&mut work as *mut RECT).cast(), 0);
            (*(lparam as *mut MINMAXINFO)).ptMinTrackSize = POINT {
                x: 820.min(work.right - work.left),
                y: 620.min(work.bottom - work.top),
            };
            0
        }
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            paint(hwnd, &*state);
            0
        }
        WM_LBUTTONDOWN => {
            let x = (lparam as u16) as i16 as i32;
            let y = ((lparam >> 16) as u16) as i16 as i32;
            if let Some(position) = (*state).layout.hit(x, y) {
                (*state).selected = position;
                update_details(&mut *state);
                SetFocus(hwnd);
                InvalidateRect(hwnd, null(), 0);
            }
            0
        }
        WM_KEYDOWN => {
            let selected = &mut (*state).selected;
            match wparam as u16 {
                VK_LEFT => selected.column = selected.column.saturating_sub(1),
                VK_RIGHT => selected.column = (selected.column + 1).min(6),
                VK_UP => selected.row = selected.row.saturating_sub(1),
                VK_DOWN => selected.row = (selected.row + 1).min(6),
                _ => return DefWindowProcW(hwnd, message, wparam, lparam),
            }
            update_details(&mut *state);
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_COMMAND if (wparam >> 16) as u32 == BN_CLICKED => {
            let id = wparam & 0xffff;
            if id == DEMO {
                let app = &mut *state;
                match app.loaded.replace_input(Input::Demo) {
                    Ok(()) => {
                        app.notice.clear();
                        app.selected = GridPos { column: 0, row: 0 };
                    }
                    Err(error) => app.notice = error,
                }
                update_details(app);
                InvalidateRect(hwnd, null(), 0);
            } else if [CAPTURE, SNAPSHOT, LIBRARY].contains(&id) {
                // The modal picker may run callbacks: no App reference spans it.
                let selected = choose_json(hwnd, id == LIBRARY);
                if let Some(path) = selected {
                    let app = &mut *state;
                    if id == LIBRARY {
                        if let Some(directory) = path.parent() {
                            app.loaded.load_library(directory);
                        }
                        app.notice =
                            "Local library selection processed; see metadata status below.".into();
                    } else {
                        let input = if id == CAPTURE {
                            Input::Capture(path)
                        } else {
                            Input::Snapshot(path)
                        };
                        match app.loaded.replace_input(input) {
                            Ok(()) => {
                                app.notice.clear();
                                app.selected = GridPos { column: 0, row: 0 };
                            }
                            Err(error) => {
                                app.notice =
                                    format!("Could not load file. Current board retained. {error}")
                            }
                        }
                    }
                    update_details(app);
                    InvalidateRect(hwnd, null(), 0);
                }
            }
            0
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

unsafe fn create_controls(hwnd: HWND, app: &mut App) -> Result<(), ()> {
    for (index, (id, label)) in [
        (DEMO, "Synthetic demo"),
        (CAPTURE, "Open capture…"),
        (SNAPSHOT, "Open snapshot…"),
        (LIBRARY, "Load local library…"),
    ]
    .into_iter()
    .enumerate()
    {
        app.buttons[index] = control(hwnd, "BUTTON", label, WS_TABSTOP, id, app.normal_font)?;
    }
    app.source = control(
        hwnd,
        "EDIT",
        "",
        ES_READONLY as u32 | ES_AUTOHSCROLL as u32,
        0,
        app.small_font,
    )?;
    app.details = control(
        hwnd,
        "EDIT",
        "",
        ES_MULTILINE as u32 | ES_READONLY as u32 | WS_VSCROLL | WS_TABSTOP,
        0,
        app.normal_font,
    )?;
    Ok(())
}
unsafe fn control(
    parent: HWND,
    class: &str,
    label: &str,
    style: u32,
    id: usize,
    font: HFONT,
) -> Result<HWND, ()> {
    let child = CreateWindowExW(
        if class == "EDIT" { WS_EX_CLIENTEDGE } else { 0 },
        wide(class).as_ptr(),
        wide(label).as_ptr(),
        WS_CHILD | WS_VISIBLE | style,
        0,
        0,
        1,
        1,
        parent,
        id as HMENU,
        GetModuleHandleW(null()),
        null(),
    );
    if child.is_null() {
        return Err(());
    }
    SendMessageW(child, WM_SETFONT, font as usize, 1);
    Ok(child)
}
unsafe fn resize(hwnd: HWND, app: &mut App) {
    let mut rect = RECT::default();
    GetClientRect(hwnd, &mut rect);
    let width = rect.right.max(1);
    let height = rect.bottom.max(1);
    let board_space = (width - 355).min(height - 220);
    app.layout = BoardLayout {
        left: 40,
        top: 190,
        tile: (board_space / 7).max(1),
    };
    let panel_left = (app.layout.left + app.layout.tile * 7 + 20).max(width - 300);
    for (index, button) in app.buttons.iter().enumerate() {
        MoveWindow(*button, 24 + index as i32 * 160, 73, 150, 33, 1);
    }
    MoveWindow(app.source, 24, 114, width - 48, 25, 1);
    MoveWindow(
        app.details,
        panel_left,
        183,
        width - panel_left - 22,
        height - 204,
        1,
    );
}

unsafe fn choose_json(hwnd: HWND, library: bool) -> Option<PathBuf> {
    let mut buffer = vec![0u16; 32768];
    if library {
        let initial = wide("manifest.json");
        buffer[..initial.len()].copy_from_slice(&initial);
    }
    let filter = wide("JSON files (*.json)\0*.json\0All files (*.*)\0*.*\0");
    let title = wide(if library {
        "Select manifest.json inside your imported library"
    } else {
        "Open a saved capture or explicit snapshot JSON"
    });
    let mut dialog = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: hwnd,
        lpstrFilter: filter.as_ptr(),
        lpstrFile: buffer.as_mut_ptr(),
        nMaxFile: buffer.len() as u32,
        lpstrTitle: title.as_ptr(),
        Flags: OFN_FILEMUSTEXIST
            | OFN_PATHMUSTEXIST
            | OFN_NOCHANGEDIR
            | OFN_EXPLORER
            | OFN_DONTADDTORECENT,
        ..Default::default()
    };
    if GetOpenFileNameW(&mut dialog) == 0 {
        return None;
    }
    let length = buffer.iter().position(|value| *value == 0)?;
    Some(PathBuf::from(OsString::from_wide(&buffer[..length])))
}

unsafe fn update_details(app: &mut App) {
    SetWindowTextW(app.source, wide(&app.loaded.source_label).as_ptr());
    let document = app.loaded.snapshot.document();
    let mut details = format!(
        "SELECTED CELL\r\nRaw column {}, row {}\r\n",
        app.selected.column, app.selected.row
    );
    if let Some(cell) = app.loaded.snapshot.cell(app.selected) {
        details.push_str(&format!("Terrain code: {}\r\n\r\n", cell.terrain_code));
        match cell.occupancy {
            Occupancy::Empty => details.push_str("Known empty in this snapshot.\r\n"),
            Occupancy::Occupied { unit_key } => {
                if let Some(unit) = app.loaded.snapshot.unit(unit_key) {
                    details.push_str(&format!(
                        "Unit key: {}\r\nCard ID: {}\r\n",
                        unit.key, unit.card_id
                    ));
                    if let Some(location) = unit.raw_location {
                        details.push_str(&format!(
                            "Raw group {}, slot {}\r\n",
                            location.array_group, location.array_slot
                        ));
                    }
                    if let Some(card) = &unit.card {
                        details.push_str(&format!(
                            "\r\n{}\r\nATK {} / DEF {}\r\nLevel {} / Deck cost {}\r\n",
                            card.name, card.attack, card.defense, card.level, card.deck_cost
                        ));
                    } else {
                        details.push_str("\r\nName/statistics unavailable. Load an optional local imported library.\r\n");
                    }
                }
            }
        }
    }
    details.push_str(&format!("\r\nSNAPSHOT\r\n{} linked occupants\r\n{} unlinked candidates excluded\r\nLP candidates: group 0 = {}, group 1 = {}\r\n\r\nSelection only. No move, summon, turn or combat commands.\r\nHuman-side and camera orientation are unverified.\r\n", document.units.len(), document.unlinked_candidates.len(), document.lp_candidates_by_raw_group[0], document.lp_candidates_by_raw_group[1]));
    if let Some(raw) = &document.raw_candidate_fields {
        details.push_str(&format!("\r\nRAW CANDIDATES\r\nMap byte: {}\r\nActive-side byte: {} (not verified turn)\r\nResource bytes: {} / {} (not verified summon points)\r\nProcess byte: {}\r\n", raw.selected_map_index, raw.active_side_byte, raw.unknown_resource_bytes[0], raw.unknown_resource_bytes[1], raw.unknown_process_b8f7));
    }
    match &document.origin {
        ViewOrigin::Synthetic { description } => details.push_str(&format!("\r\nSYNTHETIC\r\n{description}\r\n")),
        ViewOrigin::SavedCapture { state_sha256, ee_memory_sha256, .. } => details.push_str(&format!("\r\nSOURCE HASHES\r\nArchive: {state_sha256}\r\nEE RAM: {ee_memory_sha256}\r\nMetadata is supplied provenance, not game authentication.\r\n")),
        ViewOrigin::NativeSimulation { source, engine_version, rules_profile, request_sha256, checkpoint } => {
            let stage = if checkpoint == "before" {
                "BEFORE: native initialization imported from the source capture."
            } else {
                "AFTER: state computed by the experimental native core."
            };
            details.push_str(&format!("\r\nNATIVE SIMULATION\r\n{stage}\r\nEngine: {engine_version}\r\nRules profile: {rules_profile}\r\nGameplay parity is unverified. This is not a fresh emulator observation.\r\nLP values retain the source group ordering. Original raw diagnostics are omitted.\r\n\r\nINITIALIZATION REFERENCES\r\nCapture JSON: {}\r\nArchive: {}\r\nEE RAM: {}\r\nAddress interpretation: {}\r\nComputation request: {request_sha256}\r\nSupplied provenance does not authenticate game identity.\r\n", source.capture_json_sha256, source.state_sha256, source.ee_memory_sha256, source.address_profile));
        }
    }
    if let Some(library) = &app.loaded.library {
        details.push_str(&format!(
            "\r\nLocal metadata: {}\r\n",
            library.directory.display()
        ));
    }
    for warning in &app.loaded.warnings {
        details.push_str(&format!("\r\nMETADATA NOTICE\r\n{warning}\r\n"));
    }
    if !app.notice.is_empty() {
        details = format!("{}\r\n\r\n{details}", app.notice);
    }
    SetWindowTextW(app.details, wide(details).as_ptr());
}

unsafe fn fill(dc: HDC, rect: &RECT, color: u32) {
    let brush = CreateSolidBrush(color);
    FillRect(dc, rect, brush);
    DeleteObject(brush);
}
unsafe fn outline(dc: HDC, rect: &RECT, color: u32) {
    let brush = CreateSolidBrush(color);
    FrameRect(dc, rect, brush);
    DeleteObject(brush);
}
unsafe fn text(dc: HDC, value: &str, mut rect: RECT, flags: u32, color: u32) {
    SetTextColor(dc, color);
    let content = wide(value);
    DrawTextW(
        dc,
        content.as_ptr(),
        content.len() as i32 - 1,
        &mut rect,
        flags | DT_NOPREFIX,
    );
}
unsafe fn paint(hwnd: HWND, app: &App) {
    let mut paint = PAINTSTRUCT::default();
    let dc = BeginPaint(hwnd, &mut paint);
    let mut client = RECT::default();
    GetClientRect(hwnd, &mut client);
    fill(dc, &client, BACKGROUND);
    SetBkMode(dc, TRANSPARENT as i32);
    let previous_font = SelectObject(dc, app.title_font);
    text(
        dc,
        "Roses / Board Snapshot",
        RECT {
            left: 24,
            top: 17,
            right: client.right - 24,
            bottom: 49,
        },
        DT_SINGLELINE,
        TEXT,
    );
    SelectObject(dc, app.small_font);
    let label = match &app.loaded.snapshot.document().origin {
        ViewOrigin::Synthetic { .. } => "SYNTHETIC DEMO  •  Invented data  •  Viewer only, not playable",
        ViewOrigin::SavedCapture { .. } => "CAPTURED CANDIDATE FIELDS  •  Raw groups are not player identities  •  Viewer only, not playable",
        ViewOrigin::NativeSimulation { checkpoint, .. } if checkpoint == "before" => "NATIVE INITIALIZATION  •  Imported from a capture  •  Experimental, parity unverified",
        ViewOrigin::NativeSimulation { .. } => "NATIVE COMPUTED STATE  •  Experimental simulation  •  Gameplay parity unverified",
    };
    text(
        dc,
        label,
        RECT {
            left: 25,
            top: 49,
            right: client.right - 20,
            bottom: 69,
        },
        DT_SINGLELINE,
        TEXT,
    );
    let document = app.loaded.snapshot.document();
    text(
        dc,
        &format!(
            "{}  |  {} occupants  |  LP candidates G0 {} / G1 {}",
            document.label,
            document.units.len(),
            document.lp_candidates_by_raw_group[0],
            document.lp_candidates_by_raw_group[1]
        ),
        RECT {
            left: 25,
            top: 148,
            right: client.right - 20,
            bottom: 170,
        },
        DT_SINGLELINE,
        TEXT,
    );
    let colors = [
        0x00DDE8E2, 0x00C4D8BC, 0x00BCDCEB, 0x00DFD2C8, 0x00CBBAD8, 0x00E5D9B9, 0x00DFCCDF,
        0x00D4DDE3, 0x00B7C8E8, 0x00D4D9B4,
    ];
    let layout = app.layout;
    for axis in 0..7 {
        text(
            dc,
            &axis.to_string(),
            RECT {
                left: layout.left + axis * layout.tile,
                top: layout.top - 20,
                right: layout.left + (axis + 1) * layout.tile,
                bottom: layout.top,
            },
            DT_CENTER | DT_SINGLELINE,
            TEXT,
        );
        text(
            dc,
            &axis.to_string(),
            RECT {
                left: 14,
                top: layout.top + axis * layout.tile,
                right: layout.left - 6,
                bottom: layout.top + (axis + 1) * layout.tile,
            },
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            TEXT,
        );
    }
    for cell in &document.cells {
        let x = layout.left + i32::from(cell.position.column) * layout.tile;
        let y = layout.top + i32::from(cell.position.row) * layout.tile;
        let rect = RECT {
            left: x + 1,
            top: y + 1,
            right: x + layout.tile - 1,
            bottom: y + layout.tile - 1,
        };
        fill(dc, &rect, colors[usize::from(cell.terrain_code)]);
        outline(dc, &rect, 0x00ABA69F);
        if let Occupancy::Occupied { unit_key } = cell.occupancy {
            if let Some(unit) = app.loaded.snapshot.unit(unit_key) {
                let card_rect = RECT {
                    left: x + 8,
                    top: y + 7,
                    right: x + layout.tile - 8,
                    bottom: y + layout.tile - 21,
                };
                fill(dc, &card_rect, 0x00FAFCFD);
                let accent = match unit.raw_location.map(|location| location.array_group) {
                    Some(0) => 0x005849A4,
                    Some(1) => 0x00907837,
                    _ => 0x00725D45,
                };
                let stripe = RECT {
                    left: card_rect.left,
                    top: card_rect.top,
                    right: card_rect.right,
                    bottom: card_rect.top + 5,
                };
                fill(dc, &stripe, accent);
                let name = unit
                    .card
                    .as_ref()
                    .map(|card| card.name.clone())
                    .unwrap_or_else(|| format!("Card {}", unit.card_id));
                let label_width = (card_rect.right - card_rect.left - 6).max(1);
                let encoded_name = wide(&name);
                let mut name_size = SIZE::default();
                GetTextExtentPoint32W(
                    dc,
                    encoded_name.as_ptr(),
                    encoded_name.len() as i32 - 1,
                    &mut name_size,
                );
                // Small tiles use a stable short ID instead of a broken name.
                // Larger tiles permit a single-line name with native ellipsis.
                let label = if name_size.cx <= label_width || label_width >= 96 {
                    name
                } else {
                    format!("#{}", unit.card_id)
                };
                let label_top = card_rect.top + 7;
                let two_lines = card_rect.bottom - label_top >= 34;
                text(
                    dc,
                    &label,
                    RECT {
                        top: label_top,
                        left: card_rect.left + 3,
                        right: card_rect.right - 3,
                        bottom: if two_lines {
                            label_top + 18
                        } else {
                            card_rect.bottom
                        },
                    },
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
                    TEXT,
                );
                if two_lines {
                    let detail = if let Some(location) = unit.raw_location {
                        format!("G{} S{}", location.array_group, location.array_slot)
                    } else {
                        format!("#{}", unit.card_id)
                    };
                    if detail != label {
                        text(
                            dc,
                            &detail,
                            RECT {
                                top: label_top + 18,
                                left: card_rect.left + 3,
                                right: card_rect.right - 3,
                                bottom: card_rect.bottom,
                            },
                            DT_CENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
                            TEXT,
                        );
                    }
                }
            }
        }
        text(
            dc,
            &format!("T{}", cell.terrain_code),
            RECT {
                left: x + 5,
                top: y + layout.tile - 21,
                right: x + layout.tile - 5,
                bottom: y + layout.tile - 3,
            },
            DT_RIGHT | DT_SINGLELINE,
            TEXT,
        );
        if cell.position == app.selected {
            for inset in 0..3 {
                outline(
                    dc,
                    &RECT {
                        left: x + inset,
                        top: y + inset,
                        right: x + layout.tile - inset,
                        bottom: y + layout.tile - inset,
                    },
                    0x00C77A1F,
                );
            }
        }
    }
    text(
        dc,
        "Click a cell, then use arrow keys to change selection. T = raw terrain code.",
        RECT {
            left: 25,
            top: layout.top + layout.tile * 7 + 9,
            right: client.right - 25,
            bottom: client.bottom - 7,
        },
        DT_SINGLELINE,
        TEXT,
    );
    SelectObject(dc, previous_font);
    EndPaint(hwnd, &paint);
}
