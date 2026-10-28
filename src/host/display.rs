use windows::{
    Win32::{
        Graphics::Gdi::{DEVMODEW, ENUM_CURRENT_SETTINGS, EnumDisplaySettingsW},
        UI::HiDpi::GetDpiForSystem,
    },
    core::PCWSTR,
};

pub fn system_scale() -> f32 {
    unsafe { GetDpiForSystem() as f32 / 96.0 }
}

pub fn frame_interval_milliseconds() -> u32 {
    let mut mode = DEVMODEW { dmSize: size_of::<DEVMODEW>() as u16, ..Default::default() };
    let found = unsafe { EnumDisplaySettingsW(PCWSTR::null(), ENUM_CURRENT_SETTINGS, &mut mode).as_bool() };
    // A reported rate of 0 or 1 means "hardware default"; assume 60 Hz then.
    let hertz = if found && mode.dmDisplayFrequency > 1 { mode.dmDisplayFrequency } else { 60 };
    (1000.0 / hertz as f32).round().clamp(4.0, 33.0) as u32
}
