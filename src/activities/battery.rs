//! Charger and low-battery changes arrive through Windows power settings without polling.

use windows::core::GUID;

use crate::{
    activity::{Activity, Flash, Presence, SystemMessage},
    gfx::{Canvas, Color, Rect},
    page::{Context, TimerId},
    ui::{self, Icon},
};

/// GUID_ACDC_POWER_SOURCE: 0 is the mains, 1 the battery, 2 a short-term source such as a UPS.
const POWER_SOURCE_SETTING: GUID = GUID::from_u128(0x5d3e9a59_e9d5_4b00_a6bd_ff34ff516548);
const BATTERY_PERCENTAGE_SETTING: GUID = GUID::from_u128(0xa7ad8041_b45a_4cae_87a3_eecbb468a9e1);
const ON_BATTERY: u32 = 1;
// Lowest first so crossing both thresholds reports the more urgent warning.
const LOW_LEVELS: [u32; 2] = [10, 20];
const SHOW_MILLISECONDS: u32 = 3000;
const CHARGING_GREEN: Color = Color::rgb(0.19, 0.82, 0.35);
const LOW_RED: Color = Color::rgb(1.0, 0.27, 0.23);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    PluggedIn,
    Unplugged,
    Low(u32),
}

/// Turns Windows' reports into events; the first report of each kind is only the starting state.
#[derive(Default)]
struct Tracker {
    is_on_battery: Option<bool>,
    percent: Option<u32>,
}

impl Tracker {
    fn power_source(&mut self, value: u32) -> Option<Event> {
        let is_on_battery = value == ON_BATTERY;
        let previous = self.is_on_battery.replace(is_on_battery)?;
        match (previous, is_on_battery) {
            (false, true) => Some(Event::Unplugged),
            (true, false) => Some(Event::PluggedIn),
            _ => None,
        }
    }

    fn percentage(&mut self, value: u32) -> Option<Event> {
        let previous = self.percent.replace(value)?;
        if self.is_on_battery != Some(true) {
            return None;
        }
        LOW_LEVELS.into_iter().find(|&level| previous > level && value <= level).map(Event::Low)
    }
}

pub struct BatteryActivity {
    tracker: Tracker,
    event: Event,
    flash: Flash,
    width: f32,
}

impl BatteryActivity {
    pub fn new() -> Self {
        Self { tracker: Tracker::default(), event: Event::Unplugged, flash: Flash::default(), width: 0.0 }
    }

    fn percent_text(&self) -> Option<String> {
        self.tracker.percent.map(|percent| format!("{percent}%"))
    }

    fn message(&self) -> &'static str {
        match self.event {
            Event::PluggedIn => "Charging",
            Event::Unplugged => "On battery",
            Event::Low(_) => "Low battery",
        }
    }

    fn announce(&mut self, event: Event, context: &mut Context) {
        self.event = event;
        self.width = ui::message_width(context.fonts(), self.message(), self.percent_text().as_deref());
        self.flash.show(context, SHOW_MILLISECONDS);
    }
}

impl Activity for BatteryActivity {
    fn id(&self) -> &'static str {
        "battery"
    }

    fn label(&self) -> &'static str {
        "Charger and battery"
    }

    fn presence(&self) -> Option<Presence> {
        self.flash.is_visible().then_some(Presence::Transient { width: self.width })
    }

    fn draw(&self, canvas: &Canvas, area: Rect, _scale: f32, opacity: f32) {
        let (icon, tint) = match self.event {
            Event::PluggedIn => (Icon::Bolt, CHARGING_GREEN),
            Event::Unplugged => (Icon::Battery, Color::WHITE),
            Event::Low(_) => (Icon::BatteryLow, LOW_RED),
        };
        ui::draw_message(canvas, area, opacity, icon, tint, self.message(), self.percent_text().as_deref());
    }

    fn on_start(&mut self, context: &mut Context) {
        context.listen_to_power_setting(&POWER_SOURCE_SETTING);
        context.listen_to_power_setting(&BATTERY_PERCENTAGE_SETTING);
    }

    fn on_timer(&mut self, timer: TimerId, context: &mut Context) {
        self.flash.on_timer(timer, context);
    }

    fn on_system_message(&mut self, message: &SystemMessage, context: &mut Context) {
        let SystemMessage::PowerSettingChanged { setting, value } = message else { return };
        let event = if *setting == POWER_SOURCE_SETTING {
            self.tracker.power_source(*value)
        } else if *setting == BATTERY_PERCENTAGE_SETTING {
            self.tracker.percentage(*value)
        } else {
            None
        };
        if let Some(event) = event {
            self.announce(event, context);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on_battery(percent: u32) -> Tracker {
        let mut tracker = Tracker::default();
        tracker.power_source(ON_BATTERY);
        tracker.percentage(percent);
        tracker
    }

    #[test]
    fn the_starting_power_source_is_not_an_event() {
        assert_eq!(Tracker::default().power_source(0), None);
        assert_eq!(Tracker::default().power_source(ON_BATTERY), None);
    }

    #[test]
    fn unplugging_and_plugging_in_are_events() {
        let mut tracker = Tracker::default();
        tracker.power_source(0);
        assert_eq!(tracker.power_source(ON_BATTERY), Some(Event::Unplugged));
        assert_eq!(tracker.power_source(0), Some(Event::PluggedIn));
    }

    #[test]
    fn a_repeated_power_source_is_not_an_event() {
        let mut tracker = Tracker::default();
        tracker.power_source(0);
        assert_eq!(tracker.power_source(0), None);
    }

    #[test]
    fn a_short_term_source_counts_as_plugged_in() {
        let mut tracker = Tracker::default();
        tracker.power_source(ON_BATTERY);
        assert_eq!(tracker.power_source(2), Some(Event::PluggedIn));
    }

    #[test]
    fn crossing_20_percent_on_battery_warns_once() {
        let mut tracker = on_battery(22);
        assert_eq!(tracker.percentage(21), None);
        assert_eq!(tracker.percentage(20), Some(Event::Low(20)));
        assert_eq!(tracker.percentage(19), None);
    }

    #[test]
    fn crossing_10_percent_warns_again() {
        let mut tracker = on_battery(12);
        assert_eq!(tracker.percentage(10), Some(Event::Low(10)));
    }

    #[test]
    fn a_drop_through_both_levels_reports_the_lower_one() {
        let mut tracker = on_battery(25);
        assert_eq!(tracker.percentage(8), Some(Event::Low(10)));
    }

    #[test]
    fn a_low_charge_does_not_warn_while_plugged_in() {
        let mut tracker = Tracker::default();
        tracker.power_source(0);
        tracker.percentage(25);
        assert_eq!(tracker.percentage(15), None);
    }

    #[test]
    fn the_first_percentage_is_only_the_starting_point() {
        let mut tracker = Tracker::default();
        tracker.power_source(ON_BATTERY);
        assert_eq!(tracker.percentage(5), None);
    }
}
