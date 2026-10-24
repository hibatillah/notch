//! "Copied" when something is put on the clipboard. It never reads the clipboard, so nothing copied is exposed.

use std::sync::OnceLock;

use windows::{
    Win32::System::DataExchange::{CountClipboardFormats, IsClipboardFormatAvailable, RegisterClipboardFormatW},
    core::w,
};

use crate::{
    activity::{Activity, Flash, Presence, SystemMessage},
    gfx::{Canvas, Color, Rect},
    page::{Context, TimerId},
    ui::{self, Icon},
};

const SHOW_MILLISECONDS: u32 = 1200;
const MESSAGE: &str = "Copied";

pub struct ClipboardActivity {
    flash: Flash,
    width: f32,
}

impl ClipboardActivity {
    pub fn new() -> Self {
        Self { flash: Flash::default(), width: 0.0 }
    }
}

/// Password managers mark what they copy with this format to ask clipboard watchers to ignore it.
fn is_excluded_from_monitoring() -> bool {
    static FORMAT: OnceLock<u32> = OnceLock::new();
    let format =
        *FORMAT.get_or_init(|| unsafe { RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing")) });
    format != 0 && unsafe { IsClipboardFormatAvailable(format) }.is_ok()
}

/// A copy that put something on the clipboard, as opposed to a clear, and that its owner wants shown.
fn is_visible_copy() -> bool {
    (unsafe { CountClipboardFormats() }) > 0 && !is_excluded_from_monitoring()
}

impl Activity for ClipboardActivity {
    fn id(&self) -> &'static str {
        "clipboard"
    }

    fn label(&self) -> &'static str {
        "Copied"
    }

    fn presence(&self) -> Option<Presence> {
        self.flash.is_visible().then_some(Presence::Transient { width: self.width })
    }

    fn draw(&self, canvas: &Canvas, area: Rect, _scale: f32, opacity: f32) {
        ui::draw_message(canvas, area, opacity, Icon::Clipboard, Color::WHITE, MESSAGE, None);
    }

    fn on_start(&mut self, context: &mut Context) {
        self.width = ui::message_width(context.fonts(), MESSAGE, None);
        context.listen_to_clipboard();
    }

    fn on_timer(&mut self, timer: TimerId, context: &mut Context) {
        self.flash.on_timer(timer, context);
    }

    fn on_system_message(&mut self, message: &SystemMessage, context: &mut Context) {
        if *message == SystemMessage::ClipboardChanged && is_visible_copy() {
            self.flash.show(context, SHOW_MILLISECONDS);
        }
    }
}
