//! A brightness change, from the Windows slider or the brightness keys. Windows reports the laptop panel's setting
//! as a power setting, so this needs no polling and no WMI; an external monitor is not covered.

use windows::core::GUID;

use crate::{
    activity::{Activity, Flash, Presence, SystemMessage, is_change},
    gfx::{Canvas, Rect},
    page::{Context, TimerId},
    ui::{self, Icon},
};

/// GUID_DEVICE_POWER_POLICY_VIDEO_BRIGHTNESS: the brightness percentage of the active power plan.
const BRIGHTNESS_SETTING: GUID = GUID::from_u128(0xaded5e82_b909_4619_9949_f5d71dac0bcb);
const SHOW_MILLISECONDS: u32 = 1500;

pub struct BrightnessActivity {
    last_value: Option<u32>,
    percent: u32,
    flash: Flash,
}

impl BrightnessActivity {
    pub fn new() -> Self {
        Self { last_value: None, percent: 0, flash: Flash::default() }
    }
}

impl Activity for BrightnessActivity {
    fn id(&self) -> &'static str {
        "brightness"
    }

    fn label(&self) -> &'static str {
        "Brightness"
    }

    fn presence(&self) -> Option<Presence> {
        self.flash.is_visible().then_some(Presence::Transient { width: ui::LEVEL_WIDTH })
    }

    fn draw(&self, canvas: &Canvas, area: Rect, _scale: f32, opacity: f32) {
        ui::draw_level(canvas, area, opacity, Icon::Sun, self.percent as f32 / 100.0, &self.percent.to_string());
    }

    fn on_start(&mut self, context: &mut Context) {
        context.listen_to_power_setting(&BRIGHTNESS_SETTING);
    }

    fn on_timer(&mut self, timer: TimerId, context: &mut Context) {
        self.flash.on_timer(timer, context);
    }

    fn on_system_message(&mut self, message: &SystemMessage, context: &mut Context) {
        let SystemMessage::PowerSettingChanged { setting, value } = message else { return };
        if *setting == BRIGHTNESS_SETTING && is_change(&mut self.last_value, *value) {
            self.percent = (*value).min(100);
            self.flash.show(context, SHOW_MILLISECONDS);
        }
    }
}
