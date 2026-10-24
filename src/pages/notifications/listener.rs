//! Reads the notifications Windows shows, as they arrive. A program that is not a packaged app can read them with
//! `UserNotificationListener`, but its own "something changed" event cannot be subscribed to, so this waits for the
//! notification database folder to change (which happens when a toast arrives) and then reads the list.
//!
//! The list is read on a thread of its own: one read takes a few seconds when many notifications are waiting in the
//! Windows notification center, so a notification shows up in the notch a few seconds after Windows shows its banner.
//! Our own reads change the folder too, so changes right after a read are ignored.

use std::{
    collections::HashSet,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU8, Ordering},
    },
    thread,
    time::Duration,
};

use windows::{
    UI::Notifications::{
        KnownNotificationBindings,
        Management::{UserNotificationListener, UserNotificationListenerAccessStatus},
        NotificationKinds,
    },
    Win32::{
        Foundation::WAIT_OBJECT_0,
        Storage::FileSystem::{
            FILE_NOTIFY_CHANGE_LAST_WRITE, FILE_NOTIFY_CHANGE_SIZE, FindFirstChangeNotificationW,
            FindNextChangeNotification,
        },
        System::Threading::{INFINITE, WaitForSingleObject},
    },
    core::{HSTRING, Result},
};

use super::items::Item;
use crate::{log, page::Notifier};

/// Changes must stop for this long before the list is read, so one toast is one read.
const QUIET_TIME_MILLISECONDS: u32 = 300;
/// Changes in this long after a read are the read's own.
const OWN_CHANGES_TIME: Duration = Duration::from_millis(1500);

/// Whether Windows lets the notch read notifications.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Unknown = 0,
    Allowed = 1,
    Denied = 2,
}

impl Access {
    fn from_code(code: u8) -> Self {
        match code {
            1 => Access::Allowed,
            2 => Access::Denied,
            _ => Access::Unknown,
        }
    }
}

/// What the watching thread shares with the page.
pub struct Shared {
    arrived: Mutex<Vec<Item>>,
    access: AtomicU8,
}

impl Shared {
    /// The notifications that arrived since the last call, oldest first.
    pub fn take_arrived(&self) -> Vec<Item> {
        self.arrived.lock().map(|mut arrived| std::mem::take(&mut *arrived)).unwrap_or_default()
    }

    pub fn access(&self) -> Access {
        Access::from_code(self.access.load(Ordering::SeqCst))
    }
}

/// The notifications now in the Windows notification center, with their ids.
fn read_all(listener: &UserNotificationListener) -> Result<Vec<Item>> {
    let list = listener.GetNotificationsAsync(NotificationKinds::Toast)?.join()?;
    let mut items = Vec::new();
    for notification in &list {
        let app_info = notification.AppInfo()?;
        let mut texts = Vec::new();
        if let Ok(binding) =
            notification.Notification()?.Visual()?.GetBinding(&KnownNotificationBindings::ToastGeneric()?)
        {
            for text in binding.GetTextElements()? {
                texts.push(text.Text()?.to_string());
            }
        }
        let title = if texts.is_empty() { String::new() } else { texts.remove(0) };
        items.push(Item {
            id: notification.Id()?,
            app: app_info.DisplayInfo()?.DisplayName()?.to_string(),
            app_id: app_info.AppUserModelId()?.to_string(),
            title,
            body: texts.join("\n"),
            age: Duration::ZERO,
        });
    }
    Ok(items)
}

fn run(shared: &Shared, notifier: Notifier) -> Result<()> {
    let listener = UserNotificationListener::Current()?;
    let mut status = listener.GetAccessStatus()?;
    if status == UserNotificationListenerAccessStatus::Unspecified {
        status = listener.RequestAccessAsync()?.join()?;
    }
    let access = if status == UserNotificationListenerAccessStatus::Allowed { Access::Allowed } else { Access::Denied };
    shared.access.store(access as u8, Ordering::SeqCst);
    notifier.notify();
    if access != Access::Allowed {
        return Ok(());
    }

    // What is already there is not new.
    let mut seen: HashSet<u32> = read_all(&listener)?.into_iter().map(|item| item.id).collect();
    let directory = format!(r"{}\Microsoft\Windows\Notifications", std::env::var("LOCALAPPDATA").unwrap_or_default());
    let changes = unsafe {
        FindFirstChangeNotificationW(
            &HSTRING::from(directory),
            false,
            FILE_NOTIFY_CHANGE_LAST_WRITE | FILE_NOTIFY_CHANGE_SIZE,
        )?
    };
    loop {
        unsafe { WaitForSingleObject(changes, INFINITE) };
        // Wait for the changes to stop, so one toast is read once.
        loop {
            let _ = unsafe { FindNextChangeNotification(changes) };
            if unsafe { WaitForSingleObject(changes, QUIET_TIME_MILLISECONDS) } != WAIT_OBJECT_0 {
                break;
            }
        }
        let current = match read_all(&listener) {
            Ok(current) => current,
            Err(error) => {
                log::debug(&format!("notifications: cannot read the list: {error}"));
                continue;
            }
        };
        let fresh: Vec<Item> = current.iter().filter(|item| !seen.contains(&item.id)).cloned().collect();
        seen = current.into_iter().map(|item| item.id).collect();
        if !fresh.is_empty() {
            if let Ok(mut arrived) = shared.arrived.lock() {
                arrived.extend(fresh);
            }
            notifier.notify();
        }
        // The read changed the folder too; let those changes pass without reading again.
        thread::sleep(OWN_CHANGES_TIME);
        loop {
            let _ = unsafe { FindNextChangeNotification(changes) };
            if unsafe { WaitForSingleObject(changes, 0) } != WAIT_OBJECT_0 {
                break;
            }
        }
    }
}

/// Starts watching on a thread of its own.
pub fn start(notifier: Notifier) -> Arc<Shared> {
    let shared = Arc::new(Shared { arrived: Mutex::new(Vec::new()), access: AtomicU8::new(Access::Unknown as u8) });
    let worker = shared.clone();
    thread::spawn(move || {
        if let Err(error) = run(&worker, notifier) {
            log::debug(&format!("notifications: stopped: {error}"));
        }
    });
    shared
}

/// Removes a notification from the Windows notification center.
pub fn remove(id: u32) {
    if let Ok(listener) = UserNotificationListener::Current() {
        let _ = listener.RemoveNotification(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_codes_round_trip() {
        for access in [Access::Unknown, Access::Allowed, Access::Denied] {
            assert_eq!(Access::from_code(access as u8), access);
        }
        assert_eq!(Access::from_code(99), Access::Unknown);
    }
}
