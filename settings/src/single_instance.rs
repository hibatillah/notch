use windows::{
    Win32::{
        Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE},
        System::Threading::CreateMutexW,
        UI::WindowsAndMessaging::{FindWindowW, SW_RESTORE, SetForegroundWindow, ShowWindow},
    },
    core::{PCWSTR, w},
};

pub const WINDOW_TITLE: PCWSTR = w!("Notch Settings");

/// Holds the named mutex for as long as this settings window lives.
pub struct InstanceGuard(HANDLE);

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        // SAFETY: the handle came from CreateMutexW and is closed once.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// Returns `None` after focusing the existing settings window when another instance holds the mutex.
pub fn acquire() -> Option<InstanceGuard> {
    // SAFETY: plain Win32 calls; GetLastError is read right after CreateMutexW.
    unsafe {
        let mutex = CreateMutexW(None, false, w!("Local\\NotchSettings")).ok()?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(mutex);
            focus_existing_window();
            return None;
        }
        Some(InstanceGuard(mutex))
    }
}

fn focus_existing_window() {
    // SAFETY: plain Win32 calls on a window found by title.
    unsafe {
        if let Ok(existing_window) = FindWindowW(None, WINDOW_TITLE) {
            let _ = ShowWindow(existing_window, SW_RESTORE);
            let _ = SetForegroundWindow(existing_window);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_instance_is_refused_while_the_first_lives() {
        let first = acquire();
        assert!(first.is_some());
        assert!(acquire().is_none());
        drop(first);
        assert!(acquire().is_some());
    }
}
