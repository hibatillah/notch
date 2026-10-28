//! Unpackaged apps cannot subscribe to notification events; watch the database directory instead.
//! Reads run off-thread and may take seconds; ignore the directory changes caused by those reads.

use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU8, Ordering},
    },
    thread,
    time::Duration,
};

use windows::{
    ApplicationModel::AppInfo,
    Foundation::Size,
    Graphics::Imaging::{
        BitmapAlphaMode, BitmapDecoder, BitmapPixelFormat, BitmapTransform, ColorManagementMode, ExifOrientationMode,
    },
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
use crate::{gfx::ImageData, log, page::Notifier};

/// Changes must stop for this long before the list is read, so one toast is one read.
const QUIET_TIME_MILLISECONDS: u32 = 300;
/// Changes in this long after a read are the read's own.
const OWN_CHANGES_TIME: Duration = Duration::from_millis(1500);

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

pub struct Shared {
    arrived: Mutex<Vec<Item>>,
    access: AtomicU8,
}

impl Shared {
    pub fn take_arrived(&self) -> Vec<Item> {
        self.arrived.lock().map(|mut arrived| std::mem::take(&mut *arrived)).unwrap_or_default()
    }

    pub fn access(&self) -> Access {
        Access::from_code(self.access.load(Ordering::SeqCst))
    }
}

const LOGO_PIXELS: u32 = 64;

fn read_logo(app_info: &AppInfo) -> Result<ImageData> {
    let stream = app_info
        .DisplayInfo()?
        .GetLogo(Size { Width: LOGO_PIXELS as f32, Height: LOGO_PIXELS as f32 })?
        .OpenReadAsync()?
        .join()?;
    let decoder = BitmapDecoder::CreateAsync(&stream)?.join()?;
    let transform = BitmapTransform::new()?;
    transform.SetScaledWidth(LOGO_PIXELS)?;
    transform.SetScaledHeight(LOGO_PIXELS)?;
    let pixels = decoder
        .GetPixelDataTransformedAsync(
            BitmapPixelFormat::Bgra8,
            BitmapAlphaMode::Premultiplied,
            &transform,
            ExifOrientationMode::IgnoreExifOrientation,
            ColorManagementMode::DoNotColorManage,
        )?
        .join()?
        .DetachPixelData()?;
    Ok(ImageData::new(LOGO_PIXELS, LOGO_PIXELS, pixels.to_vec()))
}

type Logos = HashMap<String, Option<Arc<ImageData>>>;

fn read_all(listener: &UserNotificationListener, logos: &mut Logos) -> Result<Vec<Item>> {
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
        let app_id = app_info.AppUserModelId()?.to_string();
        let icon = logos.entry(app_id.clone()).or_insert_with(|| read_logo(&app_info).map(Arc::new).ok()).clone();
        items.push(Item {
            id: notification.Id()?,
            app: app_info.DisplayInfo()?.DisplayName()?.to_string(),
            app_id,
            icon,
            title,
            body: texts.join("\n"),
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

    let mut logos = Logos::new();
    let mut seen: HashSet<u32> = read_all(&listener, &mut logos)?.into_iter().map(|item| item.id).collect();
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
        loop {
            let _ = unsafe { FindNextChangeNotification(changes) };
            if unsafe { WaitForSingleObject(changes, QUIET_TIME_MILLISECONDS) } != WAIT_OBJECT_0 {
                break;
            }
        }
        let current = match read_all(&listener, &mut logos) {
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
        thread::sleep(OWN_CHANGES_TIME);
        loop {
            let _ = unsafe { FindNextChangeNotification(changes) };
            if unsafe { WaitForSingleObject(changes, 0) } != WAIT_OBJECT_0 {
                break;
            }
        }
    }
}

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
