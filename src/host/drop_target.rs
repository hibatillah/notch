//! OLE drops need an STA thread; the WinRT host uses MTA, so a separate overlay receives drops.
//! The overlay forwards hover while collapsed and steps aside when the notch opens.

use std::{
    cell::Cell,
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicIsize, Ordering},
    },
};

use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, POINT, POINTL, RECT, S_OK, WPARAM},
        System::{
            Com::{DVASPECT_CONTENT, FORMATETC, IDataObject, STGMEDIUM, TYMED_HGLOBAL},
            LibraryLoader::GetModuleHandleW,
            Ole::{
                CF_HDROP, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_NONE, IDropTarget, IDropTarget_Impl, OleInitialize,
                RegisterDragDrop, ReleaseStgMedium,
            },
            SystemServices::MODIFIERKEYS_FLAGS,
        },
        UI::{
            Shell::{DragQueryFileW, HDROP},
            WindowsAndMessaging::*,
        },
    },
    core::{Ref, Result, implement, w},
};

use crate::log;

pub const WM_DRAG_ENTERED: u32 = WM_APP + 10;
pub const WM_DRAG_LEFT: u32 = WM_APP + 11;
pub const WM_DRAG_DROPPED: u32 = WM_APP + 12;
/// Relative to the notch; not a mouse message, so it must not arm leave tracking on the notch.
pub const WM_POINTER_MOVED: u32 = WM_APP + 13;

const MESSAGE_HOVERING: u32 = WM_APP + 20;
const MESSAGE_RESTING_AREA: u32 = WM_APP + 21;
const MESSAGE_SUPPRESSED: u32 = WM_APP + 22;
const MESSAGE_RESET_INTERACTION: u32 = WM_APP + 23;

const DRAG_WIDTH: f32 = 720.0;
const DRAG_HEIGHT: f32 = 230.0;

static TARGET_WINDOW: AtomicIsize = AtomicIsize::new(0);
static RESTING_AREA: Mutex<Option<RECT>> = Mutex::new(None);
static DROPPED_PATHS: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

#[derive(Clone, Copy)]
struct Placement {
    notch_window: isize,
    scale: f32,
    center_x: i32,
    top: i32,
}

impl Placement {
    fn notch_window(&self) -> HWND {
        HWND(self.notch_window as *mut _)
    }
}

thread_local! {
    static PLACEMENT: Cell<Option<Placement>> = const { Cell::new(None) };
    static IS_DRAGGING: Cell<bool> = const { Cell::new(false) };
    static IS_NOTCH_HOVERED: Cell<bool> = const { Cell::new(false) };
    static IS_SUPPRESSED: Cell<bool> = const { Cell::new(false) };
}

pub fn start(notch_window: HWND, scale: f32, center_x: i32, top: i32) {
    let placement = Placement { notch_window: notch_window.0 as isize, scale, center_x, top };
    std::thread::Builder::new()
        .name("drop-target".into())
        .spawn(move || {
            if let Err(error) = run(placement) {
                log::debug(&format!("drop target unavailable: {error}"));
            }
        })
        .ok();
}

pub fn set_hovering(is_hovering: bool) {
    post(MESSAGE_HOVERING, WPARAM(usize::from(is_hovering)));
}

pub fn set_suppressed(is_suppressed: bool) {
    post(MESSAGE_SUPPRESSED, WPARAM(usize::from(is_suppressed)));
}

pub fn reset_interaction() {
    post(MESSAGE_RESET_INTERACTION, WPARAM(0));
}

pub fn set_resting_area(area: RECT) {
    let Ok(mut current) = RESTING_AREA.lock() else { return };
    let unchanged = matches!(*current, Some(old) if (old.left, old.top, old.right, old.bottom) == (area.left, area.top, area.right, area.bottom));
    if unchanged {
        return;
    }
    *current = Some(area);
    drop(current);
    post(MESSAGE_RESTING_AREA, WPARAM(0));
}

pub fn take_dropped_paths() -> Vec<PathBuf> {
    DROPPED_PATHS.lock().map(|mut paths| std::mem::take(&mut *paths)).unwrap_or_default()
}

fn post(message: u32, wparam: WPARAM) {
    let window = TARGET_WINDOW.load(Ordering::Relaxed);
    if window != 0 {
        unsafe {
            let _ = PostMessageW(Some(HWND(window as *mut _)), message, wparam, LPARAM(0));
        }
    }
}

fn post_to_notch(message: u32) {
    if let Some(placement) = PLACEMENT.get() {
        unsafe {
            let _ = PostMessageW(Some(placement.notch_window()), message, WPARAM(0), LPARAM(0));
        }
    }
}

fn run(placement: Placement) -> Result<()> {
    unsafe {
        OleInitialize(None)?;
        PLACEMENT.set(Some(placement));
        let instance = GetModuleHandleW(None)?;
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window_procedure),
            hInstance: instance.into(),
            lpszClassName: w!("NotchDropTarget"),
            ..Default::default()
        };
        RegisterClassExW(&class);
        let window = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            w!("NotchDropTarget"),
            w!("Notch drop target"),
            WS_POPUP,
            0,
            0,
            1,
            1,
            None,
            None,
            Some(instance.into()),
            None,
        )?;
        // Nearly transparent but not fully, because a fully transparent layered window is skipped by hit testing.
        SetLayeredWindowAttributes(window, Default::default(), 1, LWA_ALPHA)?;
        let target: IDropTarget = FileDropTarget.into();
        RegisterDragDrop(window, &target)?;
        TARGET_WINDOW.store(window.0 as isize, Ordering::Relaxed);
        log::debug("drop target ready");

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    Ok(())
}

fn arrange(window: HWND) {
    let Some(placement) = PLACEMENT.get() else { return };
    let area = if IS_SUPPRESSED.get() {
        None
    } else if IS_DRAGGING.get() {
        let width = (DRAG_WIDTH * placement.scale) as i32;
        Some(RECT {
            left: placement.center_x - width / 2,
            top: placement.top,
            right: placement.center_x + width / 2,
            bottom: placement.top + (DRAG_HEIGHT * placement.scale) as i32,
        })
    } else if IS_NOTCH_HOVERED.get() {
        None
    } else {
        RESTING_AREA.lock().ok().and_then(|area| *area)
    };
    unsafe {
        match area {
            Some(area) => {
                let _ = SetWindowPos(
                    window,
                    Some(HWND_TOPMOST),
                    area.left,
                    area.top,
                    area.right - area.left,
                    (area.bottom - area.top).max(4),
                    SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
            }
            None => {
                let _ = ShowWindow(window, SW_HIDE);
            }
        }
    }
}

fn forward(window: HWND, message: u32, lparam: LPARAM) {
    let Some(placement) = PLACEMENT.get() else { return };
    let (mut origin, mut bounds) = (POINT::default(), RECT::default());
    unsafe {
        let _ = windows::Win32::Graphics::Gdi::ClientToScreen(window, &mut origin);
        let _ = GetWindowRect(placement.notch_window(), &mut bounds);
    }
    let x = i32::from((lparam.0 & 0xFFFF) as u16 as i16) + origin.x;
    let y = i32::from(((lparam.0 >> 16) & 0xFFFF) as u16 as i16) + origin.y;
    // Only the notch's own area counts as pointing at it; the target may be a little larger than the pill.
    if x < bounds.left || x >= bounds.right || y < bounds.top || y >= bounds.bottom {
        return;
    }
    let relative = (((y - bounds.top) as u32 & 0xFFFF) << 16) | ((x - bounds.left) as u32 & 0xFFFF);
    unsafe {
        let _ = PostMessageW(Some(placement.notch_window()), message, WPARAM(0), LPARAM(relative as isize));
    }
}

extern "system" fn window_procedure(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match message {
        MESSAGE_RESET_INTERACTION => {
            IS_DRAGGING.set(false);
            IS_NOTCH_HOVERED.set(false);
            arrange(window);
            LRESULT(0)
        }
        MESSAGE_HOVERING => {
            IS_NOTCH_HOVERED.set(wparam.0 != 0);
            arrange(window);
            LRESULT(0)
        }
        MESSAGE_SUPPRESSED => {
            IS_SUPPRESSED.set(wparam.0 != 0);
            arrange(window);
            LRESULT(0)
        }
        MESSAGE_RESTING_AREA => {
            arrange(window);
            LRESULT(0)
        }
        WM_MOUSEMOVE | WM_RBUTTONUP => {
            forward(window, if message == WM_MOUSEMOVE { WM_POINTER_MOVED } else { message }, lparam);
            LRESULT(0)
        }
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}

fn this_window() -> HWND {
    HWND(TARGET_WINDOW.load(Ordering::Relaxed) as *mut _)
}

fn files_format() -> FORMATETC {
    FORMATETC {
        cfFormat: CF_HDROP.0,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    }
}

fn carries_files(data: Option<&IDataObject>) -> bool {
    data.is_some_and(|data| unsafe { data.QueryGetData(&files_format()) } == S_OK)
}

fn files_in(data: &IDataObject) -> Vec<PathBuf> {
    let Ok(mut medium) = (unsafe { data.GetData(&files_format()) }) else { return Vec::new() };
    let drop = HDROP(unsafe { medium.u.hGlobal.0 });
    let count = unsafe { DragQueryFileW(drop, u32::MAX, None) };
    let mut paths = Vec::new();
    for index in 0..count {
        let length = unsafe { DragQueryFileW(drop, index, None) } as usize;
        let mut buffer = vec![0u16; length + 1];
        let written = unsafe { DragQueryFileW(drop, index, Some(&mut buffer)) } as usize;
        paths.push(PathBuf::from(String::from_utf16_lossy(&buffer[..written])));
    }
    unsafe { ReleaseStgMedium(&mut medium as *mut STGMEDIUM) };
    paths
}

fn end_drag() {
    IS_DRAGGING.set(false);
    arrange(this_window());
}

#[implement(IDropTarget)]
struct FileDropTarget;

impl IDropTarget_Impl for FileDropTarget_Impl {
    fn DragEnter(
        &self,
        data: Ref<'_, IDataObject>,
        _keys: MODIFIERKEYS_FLAGS,
        _point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> Result<()> {
        let accepts = carries_files(data.as_ref()) && !crate::platform::is_dragging_out();
        log::debug(&format!("drop target: drag entered, files: {accepts}"));
        unsafe { *effect = if accepts { DROPEFFECT_COPY } else { DROPEFFECT_NONE } };
        if accepts {
            IS_DRAGGING.set(true);
            arrange(this_window());
            post_to_notch(WM_DRAG_ENTERED);
        }
        Ok(())
    }

    fn DragOver(&self, _keys: MODIFIERKEYS_FLAGS, _point: &POINTL, effect: *mut DROPEFFECT) -> Result<()> {
        unsafe { *effect = if IS_DRAGGING.get() { DROPEFFECT_COPY } else { DROPEFFECT_NONE } };
        Ok(())
    }

    fn DragLeave(&self) -> Result<()> {
        log::debug("drop target: drag left");
        if IS_DRAGGING.get() {
            end_drag();
            post_to_notch(WM_DRAG_LEFT);
        }
        Ok(())
    }

    fn Drop(
        &self,
        data: Ref<'_, IDataObject>,
        _keys: MODIFIERKEYS_FLAGS,
        _point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> Result<()> {
        let paths = data.as_ref().map(files_in).unwrap_or_default();
        unsafe { *effect = if paths.is_empty() { DROPEFFECT_NONE } else { DROPEFFECT_COPY } };
        log::debug(&format!("drop target: dropped {} files", paths.len()));
        end_drag();
        if let Ok(mut dropped) = DROPPED_PATHS.lock() {
            *dropped = paths;
        }
        post_to_notch(WM_DRAG_DROPPED);
        Ok(())
    }
}
