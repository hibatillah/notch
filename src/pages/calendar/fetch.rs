//! Calendar requests run off-thread and notify the page when complete.

use std::sync::{Arc, Mutex};

use windows::{Foundation::Uri, Web::Http::HttpClient, core::HSTRING};

use crate::page::Notifier;

pub type Outcome = Arc<Mutex<Option<Result<String, String>>>>;

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
