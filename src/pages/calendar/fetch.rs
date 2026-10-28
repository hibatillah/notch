//! Calendar requests run off-thread and notify the page when complete.

use std::sync::{Arc, Mutex};

use windows::{Foundation::Uri, Web::Http::HttpClient, core::HSTRING};

use crate::page::Notifier;

pub type Outcome = Arc<Mutex<Option<Result<String, String>>>>;

/// The address to download from what the user copied: a `webcal://` address is the same address over `https://`.
/// `None` when the text is not a web address.
pub fn normalize_link(text: &str) -> Option<String> {
    let line = text.lines().next()?.trim();
    let link = match line.strip_prefix("webcal://") {
        Some(rest) => format!("https://{rest}"),
        None => line.to_owned(),
    };
    let has_scheme = link.starts_with("https://") || link.starts_with("http://");
    (has_scheme && link.len() > "https://x.y".len() && !link.contains(char::is_whitespace)).then_some(link)
}

fn download(link: &str) -> Result<String, String> {
    let describe = |error: windows::core::Error| error.message();
    let client = HttpClient::new().map_err(describe)?;
    let uri = Uri::CreateUri(&HSTRING::from(link)).map_err(describe)?;
    let text = client.GetStringAsync(&uri).map_err(describe)?.join().map_err(describe)?;
    Ok(text.to_string())
}

pub fn start(link: String, outcome: Outcome, notifier: Notifier) {
    std::thread::spawn(move || {
        let result = download(&link);
        if let Ok(mut slot) = outcome.lock() {
            *slot = Some(result);
        }
        notifier.notify();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_web_address_is_kept_and_a_webcal_address_becomes_https() {
        assert_eq!(
            normalize_link("https://calendar.example.com/ical/secret/basic.ics"),
            Some("https://calendar.example.com/ical/secret/basic.ics".to_owned())
        );
        assert_eq!(
            normalize_link("  webcal://example.com/cal.ics \r\n"),
            Some("https://example.com/cal.ics".to_owned())
        );
    }

    #[test]
    fn only_the_first_line_counts() {
        assert_eq!(
            normalize_link("https://example.com/a.ics\nsecond line"),
            Some("https://example.com/a.ics".to_owned())
        );
    }

    #[test]
    fn text_that_is_not_an_address_is_refused() {
        assert_eq!(normalize_link(""), None);
        assert_eq!(normalize_link("hello world"), None);
        assert_eq!(normalize_link("ftp://example.com/a.ics"), None);
        assert_eq!(normalize_link("https://"), None);
        assert_eq!(normalize_link("https://example.com/with space.ics"), None);
    }
}
