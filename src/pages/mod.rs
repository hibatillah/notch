//! Every page the notch can show. This list is the only place pages are registered.
//!
//! To add a page: create a folder here with a type that implements `Page`, declare the module below and add the
//! type to `all()`. To remove a page: delete its folder, its `mod` line and its entry in `all()`.
//! When several pages want to be shown, the one with the highest priority wins; ties go to the earlier entry.

mod calendar;
mod clock;
mod media;
mod notifications;
mod shelf;
mod stats;
mod timer;

use crate::page::Page;

pub fn all() -> Vec<Box<dyn Page>> {
    vec![
        Box::new(clock::ClockPage::new()),
        Box::new(media::MediaPage::new()),
        Box::new(notifications::NotificationsPage::new()),
        Box::new(shelf::ShelfPage::new()),
        Box::new(calendar::CalendarPage::new()),
        Box::new(timer::TimerPage::new()),
        Box::new(stats::StatsPage::new()),
    ]
}
