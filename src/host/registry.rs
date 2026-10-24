use super::Settings;
use crate::{log, page::Page};

/// Keeps the pages named by the `NOTCH_PAGES` environment variable or the `pages` setting (comma-separated page ids);
/// with neither set, every registered page stays on.
pub fn enabled_pages(pages: Vec<Box<dyn Page>>, settings: &Settings) -> Vec<Box<dyn Page>> {
    let wanted = std::env::var("NOTCH_PAGES").ok().or_else(|| settings.get_string("pages").map(str::to_owned));
    filter_pages(pages, wanted.as_deref())
}

fn filter_pages(pages: Vec<Box<dyn Page>>, wanted: Option<&str>) -> Vec<Box<dyn Page>> {
    let Some(wanted) = wanted else { return pages };
    let ids: Vec<&str> = wanted.split(',').map(str::trim).filter(|id| !id.is_empty()).collect();
    let (kept, dropped): (Vec<_>, Vec<_>) = pages.into_iter().partition(|page| ids.contains(&page.id()));
    if kept.is_empty() {
        log::debug(&format!("no registered page matches {wanted:?}; keeping all pages"));
        return dropped;
    }
    for page in &dropped {
        log::debug(&format!("page {} is switched off", page.id()));
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        gfx::{Canvas, Size},
        page::{Frame, Mode, PageSizes, Priority},
    };

    struct Named(&'static str);

    impl Page for Named {
        fn id(&self) -> &'static str {
            self.0
        }
        fn priority(&self, _mode: Mode) -> Option<Priority> {
            None
        }
        fn sizes(&self) -> PageSizes {
            PageSizes { resting_width: 0.0, expanded: Size::new(0.0, 0.0) }
        }
        fn draw_resting(&self, _canvas: &Canvas, _frame: &Frame, _opacity: f32) {}
        fn draw_expanded(&self, _canvas: &Canvas, _frame: &Frame, _opacity: f32) {}
    }

    fn pages() -> Vec<Box<dyn Page>> {
        vec![Box::new(Named("clock")), Box::new(Named("media")), Box::new(Named("timer"))]
    }

    fn ids(pages: &[Box<dyn Page>]) -> Vec<&'static str> {
        pages.iter().map(|page| page.id()).collect()
    }

    #[test]
    fn without_a_list_every_page_stays_on() {
        assert_eq!(ids(&filter_pages(pages(), None)), vec!["clock", "media", "timer"]);
    }

    #[test]
    fn only_listed_pages_stay_on() {
        assert_eq!(ids(&filter_pages(pages(), Some("clock"))), vec!["clock"]);
    }

    #[test]
    fn the_list_is_trimmed_and_registration_order_is_kept() {
        assert_eq!(ids(&filter_pages(pages(), Some(" timer , clock "))), vec!["clock", "timer"]);
    }

    #[test]
    fn a_list_matching_nothing_keeps_every_page() {
        assert_eq!(ids(&filter_pages(pages(), Some("nope"))), vec!["clock", "media", "timer"]);
        assert_eq!(ids(&filter_pages(pages(), Some(""))), vec!["clock", "media", "timer"]);
    }
}
