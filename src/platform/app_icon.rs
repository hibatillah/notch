//! Finds the icon of an app from its Windows app identifier.

use std::ffi::c_void;

use windows::{
    Win32::{
        Foundation::{CloseHandle, SIZE},
        Graphics::Gdi::{
            BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteObject, GetDC, GetDIBits, GetObjectW,
            HBITMAP, ReleaseDC,
        },
        System::{
            Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize},
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
            },
            Threading::{
                OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
            },
        },
        UI::Shell::{IShellItem, IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_ICONONLY},
    },
    core::{HSTRING, Interface, PWSTR},
};

use crate::gfx::ImageData;

const REQUESTED_PIXELS: i32 = 256;
/// Retain roughly twice the drawn size for smooth downscaling.
const STORED_PIXELS: u32 = 64;

/// Loads the icon of an app, from its Windows app identifier.
/// Packaged apps resolve through the Apps folder; classic apps (such as `Spotify.exe`) through their running process.
pub fn load(app_user_model_id: &str) -> Option<ImageData> {
    if app_user_model_id.is_empty() {
        return None;
    }
    unsafe {
        // The shell image factory needs COM initialised on the calling thread.
        let initialised = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
        let bitmap = image_for(&format!("shell:AppsFolder\\{app_user_model_id}"))
            .or_else(|| executable_path(app_user_model_id).and_then(|path| image_for(&path)));
        let artwork = bitmap.and_then(|bitmap| read_pixels(bitmap)).map(|icon| icon.downscaled(STORED_PIXELS));
        if initialised {
            CoUninitialize();
        }
        artwork
    }
}

unsafe fn image_for(parsing_name: &str) -> Option<HBITMAP> {
    unsafe {
        let item: IShellItem = SHCreateItemFromParsingName(&HSTRING::from(parsing_name), None).ok()?;
        let factory: IShellItemImageFactory = item.cast().ok()?;
        factory.GetImage(SIZE { cx: REQUESTED_PIXELS, cy: REQUESTED_PIXELS }, SIIGBF_ICONONLY).ok()
    }
}

unsafe fn executable_path(file_name: &str) -> Option<String> {
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()?;
        let mut entry = PROCESSENTRY32W { dwSize: size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut path = None;
        let mut more = Process32FirstW(snapshot, &mut entry).is_ok();
        while more && path.is_none() {
            let length = entry.szExeFile.iter().position(|&unit| unit == 0).unwrap_or(entry.szExeFile.len());
            if String::from_utf16_lossy(&entry.szExeFile[..length]).eq_ignore_ascii_case(file_name) {
                path = process_image_path(entry.th32ProcessID);
            }
            more = Process32NextW(snapshot, &mut entry).is_ok();
        }
        let _ = CloseHandle(snapshot);
        path
    }
}

unsafe fn process_image_path(process_id: u32) -> Option<String> {
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id).ok()?;
        let mut buffer = [0u16; 1024];
        let mut length = buffer.len() as u32;
        let result = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buffer.as_mut_ptr()), &mut length);
        let _ = CloseHandle(process);
        result.ok()?;
        Some(String::from_utf16_lossy(&buffer[..length as usize]))
    }
}

unsafe fn read_pixels(bitmap: HBITMAP) -> Option<ImageData> {
    unsafe {
        let mut header = BITMAP::default();
        GetObjectW(bitmap.into(), size_of::<BITMAP>() as i32, Some((&raw mut header).cast::<c_void>()));
        let (width, height) = (header.bmWidth, header.bmHeight);
        if width <= 0 || height <= 0 {
            let _ = DeleteObject(bitmap.into());
            return None;
        }

        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        let screen = GetDC(None);
        let rows =
            GetDIBits(screen, bitmap, 0, height as u32, Some(pixels.as_mut_ptr().cast()), &mut info, DIB_RGB_COLORS);
        ReleaseDC(None, screen);
        let _ = DeleteObject(bitmap.into());
        if rows == 0 {
            return None;
        }

        normalise_alpha(&mut pixels);
        Some(ImageData::new(width as u32, height as u32, pixels))
    }
}

/// Shell bitmaps can be straight-alpha or lack alpha; Direct2D needs premultiplied.
fn normalise_alpha(pixels: &mut [u8]) {
    if pixels.as_chunks::<4>().0.iter().all(|pixel| pixel[3] == 0) {
        pixels.as_chunks_mut::<4>().0.iter_mut().for_each(|pixel| pixel[3] = 255);
        return;
    }
    let is_straight_alpha =
        pixels.as_chunks::<4>().0.iter().any(|pixel| pixel[0] > pixel[3] || pixel[1] > pixel[3] || pixel[2] > pixel[3]);
    if is_straight_alpha {
        for pixel in pixels.as_chunks_mut::<4>().0 {
            let alpha = u16::from(pixel[3]);
            for channel in &mut pixel[..3] {
                *channel = ((u16::from(*channel) * alpha + 127) / 255) as u8;
            }
        }
    }
}
