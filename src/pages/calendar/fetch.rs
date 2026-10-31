//! Calendar requests run off-thread and notify the page when complete.

use std::sync::{Arc, Mutex};

use windows::{Foundation::Uri, Web::Http::HttpClient, core::HSTRING};

use crate::page::Notifier;

/// The address a download was started for, so a result that arrives after the address changed is dropped.
pub type Outcome = Arc<Mutex<Option<(String, Result<String, String>)>>>;

/// Apple and Outlook share `webcal://` addresses; the download itself is plain HTTPS.
fn request_address(link: &str) -> String {
    match link.get(..9) {
        Some(scheme) if scheme.eq_ignore_ascii_case("webcal://") => format!("https://{}", &link[9..]),
        _ => link.to_owned(),
    }
}

fn download(link: &str) -> Result<String, String> {
    let link = request_address(link);
    let describe = |error: windows::core::Error| error.message();
    let client = HttpClient::new().map_err(describe)?;
    let uri = Uri::CreateUri(&HSTRING::from(link.as_str())).map_err(describe)?;
    let text = client.GetStringAsync(&uri).map_err(describe)?.join().map_err(describe)?;
    Ok(text.to_string())
}

pub fn start(link: String, outcome: Outcome, notifier: Notifier) {
    std::thread::spawn(move || {
        let result = download(&link);
        if let Ok(mut slot) = outcome.lock() {
            *slot = Some((link, result));
        }
        notifier.notify();
    });
}

#[cfg(test)]
mod tests {
    use super::request_address;

    #[test]
    fn a_webcal_address_is_requested_over_https() {
        assert_eq!(request_address("webcal://example.com/a.ics"), "https://example.com/a.ics");
        assert_eq!(request_address("WebCal://example.com/a.ics"), "https://example.com/a.ics");
        assert_eq!(request_address("https://example.com/a.ics"), "https://example.com/a.ics");
        assert_eq!(request_address("web"), "web");
    }
}
