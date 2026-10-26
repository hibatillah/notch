//! Small Windows actions that pages need: opening a file or a link, reading text from the clipboard, the double-click time, dragging a file out of the notch, and getting the picture Explorer shows for a file. Each is
//! a thin wrapper over one Windows call.

use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use windows::{
    Win32::{
        Foundation::{HGLOBAL, SIZE},
        Graphics::Gdi::{
            BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteObject, GetDC, GetDIBits, GetObjectW, HGDIOBJ,
            ReleaseDC,
        },
        System::{
            Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize, IBindCtx, IDataObject},
            DataExchange::{CloseClipboard, GetClipboardData, OpenClipboard},
            Memory::{GlobalLock, GlobalUnlock},
            Ole::{DROPEFFECT_COPY, DROPEFFECT_NONE, IDropSource, OleInitialize, OleUninitialize},
            Threading::{AttachThreadInput, GetCurrentThreadId},
        },
        UI::{
            Input::KeyboardAndMouse::{GetAsyncKeyState, GetDoubleClickTime, VK_LBUTTON},
            Shell::{
                BHID_DataObject, IShellItem, IShellItemImageFactory, SHCreateItemFromParsingName, SHDoDragDrop,
                SIIGBF_RESIZETOFIT, ShellExecuteW,
            },
            WindowsAndMessaging::SW_SHOWNORMAL,
        },
    },
    core::{HSTRING, PCWSTR, w},
};

use crate::gfx::ImageData;

/// The clipboard format for UTF-16 text.
const CF_UNICODETEXT: u32 = 13;

/// True while a file dragged out of the notch is being carried, so the notch's own drop target ignores it.
static IS_DRAGGING_OUT: AtomicBool = AtomicBool::new(false);

pub fn is_dragging_out() -> bool {
    IS_DRAGGING_OUT.load(Ordering::Relaxed)
}

/// Opens a file, folder or web address with the program Windows has for it.
pub fn open(target: &str) {
    unsafe { ShellExecuteW(None, w!("open"), &HSTRING::from(target), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL) };
}

/// How long the user allows between the two clicks of a double click.
pub fn double_click_time() -> Duration {
    Duration::from_millis(u64::from(unsafe { GetDoubleClickTime() }))
}

/// The text on the clipboard, if there is any.
pub fn paste_text() -> Option<String> {
    unsafe {
        OpenClipboard(None).ok()?;
        let text = (|| {
            let handle = GetClipboardData(CF_UNICODETEXT).ok()?;
            let memory = HGLOBAL(handle.0);
            let source = GlobalLock(memory).cast::<u16>();
            if source.is_null() {
                return None;
            }
            let length = (0..).take_while(|&offset| *source.add(offset) != 0).count();
            let text = String::from_utf16_lossy(std::slice::from_raw_parts(source, length));
            let _ = GlobalUnlock(memory);
            Some(text)
        })();
        let _ = CloseClipboard();
        text
    }
}

/// Keeps COM initialized on the current thread until dropped.
struct ComApartment(bool);

impl ComApartment {
    fn enter() -> Self {
        Self(unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok())
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}

/// The picture Explorer shows for `path` (a preview of an image, the icon of a program, the icon of a folder), about
/// `pixels` wide, in premultiplied BGRA. Slow for some files, so call it from a background thread.
pub fn file_preview(path: &Path, pixels: i32) -> Option<ImageData> {
    let _apartment = ComApartment::enter();
    unsafe {
        let item: IShellItemImageFactory =
            SHCreateItemFromParsingName(&HSTRING::from(path.as_os_str()), None::<&IBindCtx>).ok()?;
        let bitmap = item.GetImage(SIZE { cx: pixels, cy: pixels }, SIIGBF_RESIZETOFIT).ok()?;
        let handle = HGDIOBJ(bitmap.0);
        let mut info = BITMAP::default();
        let described = GetObjectW(handle, size_of::<BITMAP>() as i32, Some((&mut info as *mut BITMAP).cast()));
        let (width, height) = (info.bmWidth, info.bmHeight);
        let image = (described != 0 && width > 0 && height > 0)
            .then(|| {
                let mut header = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: width,
                        // Negative height: the rows run from the top.
                        biHeight: -height,
                        biPlanes: 1,
                        biBitCount: 32,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                let mut bytes = vec![0u8; width as usize * height as usize * 4];
                let device = GetDC(None);
                let rows = GetDIBits(
                    device,
                    bitmap,
                    0,
                    height as u32,
                    Some(bytes.as_mut_ptr().cast()),
                    &mut header,
                    DIB_RGB_COLORS,
                );
                ReleaseDC(None, device);
                (rows != 0).then(|| {
                    premultiplied(&mut bytes);
                    ImageData::new(width as u32, height as u32, bytes)
                })
            })
            .flatten();
        let _ = DeleteObject(handle);
        image
    }
}

/// Makes BGRA pixels from the shell premultiplied: some come with no alpha at all (a photo), and some with colors that
/// are not yet multiplied by it.
fn premultiplied(bytes: &mut [u8]) {
    let (pixels, _) = bytes.as_chunks_mut::<4>();
    if pixels.iter().all(|pixel| pixel[3] == 0) {
        pixels.iter_mut().for_each(|pixel| pixel[3] = 255);
        return;
    }
    let is_premultiplied = pixels.iter().all(|pixel| pixel[..3].iter().all(|&channel| channel <= pixel[3]));
    if is_premultiplied {
        return;
    }
    for pixel in pixels {
        let alpha = u32::from(pixel[3]);
        for channel in &mut pixel[..3] {
            *channel = ((u32::from(*channel) * alpha + 127) / 255) as u8;
        }
    }
}

/// Drags `path` out of the notch like a file dragged from Explorer, on a thread of its own, because dragging needs a
/// single-threaded apartment and the notch's thread is not one. `finished` gets the path and true when something took
/// a copy of the file, false when the drag was cancelled. Returns false when the drag could not
/// start at all.
pub fn drag_file_out(path: PathBuf, finished: impl FnOnce(PathBuf, bool) + Send + 'static) -> bool {
    // A drag follows the button, so with it already up there is nothing to carry.
    if !is_left_button_down() {
        return false;
    }
    let notch_thread = unsafe { GetCurrentThreadId() };
    IS_DRAGGING_OUT.store(true, Ordering::Relaxed);
    let spawned = std::thread::Builder::new().name("shelf-drag".into()).spawn(move || {
        let was_taken = drag_out(&path, notch_thread);
        IS_DRAGGING_OUT.store(false, Ordering::Relaxed);
        finished(path, was_taken);
    });
    if spawned.is_err() {
        IS_DRAGGING_OUT.store(false, Ordering::Relaxed);
    }
    spawned.is_ok()
}

fn is_left_button_down() -> bool {
    unsafe { GetAsyncKeyState(i32::from(VK_LBUTTON.0)) < 0 }
}

/// Shares the input state of `notch_thread` with the current thread until dropped. The button went down in the notch's
/// window, so only a thread that shares the notch's input sees it go up, which is what ends the drag; without it the
/// drag never ends and holds the mouse.
struct SharedInput {
    own_thread: u32,
    notch_thread: u32,
    is_attached: bool,
}

impl SharedInput {
    fn attach(notch_thread: u32) -> Self {
        let own_thread = unsafe { GetCurrentThreadId() };
        let is_attached = unsafe { AttachThreadInput(own_thread, notch_thread, true) }.as_bool();
        Self { own_thread, notch_thread, is_attached }
    }
}

impl Drop for SharedInput {
    fn drop(&mut self) {
        if self.is_attached {
            let _ = unsafe { AttachThreadInput(self.own_thread, self.notch_thread, false) };
        }
    }
}

/// Runs the drag; true when it ended in a drop that took a copy of the file. Only copying is offered, so that a drop on
/// the desktop, a folder or the Recycle Bin can never move or delete the original.
fn drag_out(path: &Path, notch_thread: u32) -> bool {
    unsafe {
        if OleInitialize(None).is_err() {
            return false;
        }
        // Attached once OLE has given this thread its message queue.
        let input = SharedInput::attach(notch_thread);
        let taken = (|| {
            let item: IShellItem =
                SHCreateItemFromParsingName(&HSTRING::from(path.as_os_str()), None::<&IBindCtx>).ok()?;
            let data: IDataObject = item.BindToHandler(None::<&IBindCtx>, &BHID_DataObject).ok()?;
            let effect = SHDoDragDrop(None, &data, None::<&IDropSource>, DROPEFFECT_COPY).ok()?;
            Some(effect != DROPEFFECT_NONE)
        })();
        drop(input);
        OleUninitialize();
        taken.unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixels_with_no_alpha_become_opaque() {
        let mut bytes = vec![10, 20, 30, 0, 40, 50, 60, 0];
        premultiplied(&mut bytes);
        assert_eq!(bytes, vec![10, 20, 30, 255, 40, 50, 60, 255]);
    }

    #[test]
    fn pixels_already_multiplied_by_their_alpha_stay_as_they_are() {
        let mut bytes = vec![10, 20, 30, 128, 0, 0, 0, 0];
        premultiplied(&mut bytes);
        assert_eq!(bytes, vec![10, 20, 30, 128, 0, 0, 0, 0]);
    }

    #[test]
    fn pixels_brighter_than_their_alpha_are_multiplied() {
        let mut bytes = vec![200, 100, 0, 128];
        premultiplied(&mut bytes);
        assert_eq!(bytes, vec![100, 50, 0, 128]);
    }

    #[test]
    fn a_missing_file_has_no_preview() {
        assert!(file_preview(Path::new(r"C:\no\such\folder\file.png"), 64).is_none());
    }
}
