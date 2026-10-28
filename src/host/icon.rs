use windows::{
    Win32::{
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{DestroyIcon, HICON, IMAGE_ICON, LR_DEFAULTCOLOR, LoadImageW},
    },
    core::{Result, w},
};

pub struct Icon(HICON);

impl Icon {
    pub fn load(size: i32) -> Result<Self> {
        let instance = unsafe { GetModuleHandleW(None)? };
        let handle =
            unsafe { LoadImageW(Some(instance.into()), w!("NOTCH_ICON"), IMAGE_ICON, size, size, LR_DEFAULTCOLOR)? };
        Ok(Self(HICON(handle.0)))
    }

    pub fn handle(&self) -> HICON {
        self.0
    }
}

impl Drop for Icon {
    fn drop(&mut self) {
        if let Err(error) = unsafe { DestroyIcon(self.0) } {
            crate::log::debug(&format!("could not release app icon: {error}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::{
        Graphics::Gdi::{BITMAP, DeleteObject, GetObjectW},
        UI::WindowsAndMessaging::{GetIconInfo, ICONINFO},
    };

    #[test]
    fn the_embedded_logo_loads_at_windows_icon_sizes() {
        for size in [16, 20, 24, 32, 40, 48, 64, 96, 128, 256] {
            let icon = Icon::load(size).expect("embedded orange logo");
            let mut info = ICONINFO::default();
            unsafe { GetIconInfo(icon.handle(), &mut info) }.expect("icon bitmaps");
            let mut bitmap = BITMAP::default();
            let copied =
                unsafe { GetObjectW(info.hbmColor.into(), size_of::<BITMAP>() as i32, Some((&raw mut bitmap).cast())) };
            unsafe { DeleteObject(info.hbmColor.into()) }.ok().expect("release color bitmap");
            unsafe { DeleteObject(info.hbmMask.into()) }.ok().expect("release mask bitmap");
            assert_ne!(copied, 0);
            assert_eq!((bitmap.bmWidth, bitmap.bmHeight), (size, size));
            assert_eq!(bitmap.bmBitsPixel, 32);
        }
    }
}
