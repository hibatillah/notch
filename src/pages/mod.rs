//! Every page the notch can show. This list is the only place pages are registered.
//!
//! To add a page: create a folder here with a type that implements `Page`, declare the module below and add the
//! type to `all()`. To remove a page: delete its folder, its `mod` line and its entry in `all()`.
//! When several pages want to be shown, the one with the highest priority wins; ties go to the earlier entry.

mod calendar;
mod media;
mod shelf;
mod stats;
mod timer;

use crate::page::Page;

pub fn all() -> Vec<Box<dyn Page>> {
    vec![
        Box::new(media::MediaPage::new()),
        Box::new(shelf::ShelfPage::new()),
        Box::new(calendar::CalendarPage::new()),
        Box::new(timer::TimerPage::new()),
        Box::new(stats::StatsPage::new()),
    ]
}

/// Which pages of a sample set carry example content, for rendering the notch off-screen.
#[cfg(test)]
#[derive(Clone, Copy, Default)]
pub struct SampleOptions {
    /// The media page plays a track: `(lyrics on, title shown after a track change)`; `None` leaves it empty.
    pub media: Option<(bool, bool)>,
    pub timer: bool,
    pub calendar: bool,
    /// Files are being dragged over the notch.
    pub file_drag: bool,
}

/// Every page, in registration order, with example content where `options` asks for it and for the pages that are
/// always filled in (the shelf, the stats).
#[cfg(test)]
pub fn samples(fonts: &crate::gfx::Fonts, directory: &std::path::Path, options: SampleOptions) -> Vec<Box<dyn Page>> {
    vec![
        Box::new(match options.media {
            Some((lyrics_on, show_title)) => media::MediaPage::sample(fonts, lyrics_on, show_title),
            None => media::MediaPage::sample_idle(fonts),
        }),
        Box::new(shelf::ShelfPage::sample(directory, fonts, options.file_drag)),
        Box::new(if options.calendar { calendar::CalendarPage::sample(fonts) } else { calendar::CalendarPage::new() }),
        Box::new(if options.timer { timer::TimerPage::sample(fonts) } else { timer::TimerPage::new() }),
        Box::new(stats::StatsPage::sample()),
    ]
}
