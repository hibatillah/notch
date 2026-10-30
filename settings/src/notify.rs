use windows::{
    Win32::UI::WindowsAndMessaging::{FindWindowW, PostMessageW, WM_APP},
    core::w,
};

/// Same value as `WM_SETTINGS_CHANGED` in the notch's `host/window.rs`.
const WM_SETTINGS_CHANGED: u32 = WM_APP + 30;

pub fn tell_notch_to_reload() {
    // SAFETY: plain Win32 lookups; a missing window is an Err and means no notch is running.
    unsafe {
        if let Ok(notch_window) = FindWindowW(w!("NotchWindow"), None) {
            let _ = PostMessageW(Some(notch_window), WM_SETTINGS_CHANGED, Default::default(), Default::default());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notifying_without_a_running_notch_does_nothing() {
        tell_notch_to_reload();
    }
}
