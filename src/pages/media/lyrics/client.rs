//! Looks up synced lyrics on LRCLIB (https://lrclib.net), a free lyrics database that needs no account.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

use windows::{
    Data::Json::{JsonArray, JsonObject},
    Foundation::Uri,
    Web::Http::{HttpClient, HttpStatusCode},
    core::{HSTRING, Result, h},
};

use super::{Lyrics, parse_synced_lyrics};

const USER_AGENT: &str = "notch/0.1 (personal desktop widget)";
/// Waits between retries after the first failed request, about 18 s in total.
const RETRY_DELAYS: [Duration; 4] = [
    Duration::from_millis(1500),
    Duration::from_millis(3000),
    Duration::from_millis(5000),
    Duration::from_millis(8000),
];
const SEARCH_DURATION_TOLERANCE_SECONDS: f64 = 4.0;

/// Looks up synced lyrics for a track. `Ok(None)` means the service has none; `Err` means it could not be reached.
///
/// The exact-match request and the search run at the same time and the first one that finds lyrics wins, so a slow or
/// failing exact match does not hold up the search.
pub fn fetch(artist: &str, title: &str, album: &str, duration_seconds: i64) -> Result<Option<Lyrics>> {
    let client = HttpClient::new()?;
    client.DefaultRequestHeaders()?.UserAgent()?.TryParseAdd(&HSTRING::from(USER_AGENT))?;

    let mut exact_url = format!(
        "https://lrclib.net/api/get?artist_name={}&track_name={}",
        percent_encode(artist),
        percent_encode(title),
    );
    if !album.is_empty() {
        exact_url.push_str(&format!("&album_name={}", percent_encode(album)));
    }
    if duration_seconds > 0 {
        exact_url.push_str(&format!("&duration={duration_seconds}"));
    }
    // Player titles often carry suffixes the lyric database lacks ("(feat. X)", "- Remastered"), so search by the plain name.
    let search_url = format!(
        "https://lrclib.net/api/search?track_name={}&artist_name={}",
        percent_encode(&clean_title(title)),
        percent_encode(first_artist(artist)),
    );

    let is_settled = Arc::new(AtomicBool::new(false));
    let (sender, receiver) = mpsc::channel();
    let exact_sender = sender.clone();
    let (exact_client, exact_settled) = (client.clone(), is_settled.clone());
    thread::spawn(move || {
        let found = request(&exact_client, &exact_url, &exact_settled).map(|body| {
            body.and_then(|body| JsonObject::Parse(&body).ok().and_then(|object| lyrics_from_object(&object)))
        });
        let _ = exact_sender.send(found);
    });
    let search_settled = is_settled.clone();
    thread::spawn(move || {
        let found = request(&client, &search_url, &search_settled)
            .map(|body| body.and_then(|body| best_search_match(&body, duration_seconds)));
        let _ = sender.send(found);
    });

    let result = first_found(receiver.iter().take(2));
    // Whichever request is still retrying has nothing left to do.
    is_settled.store(true, Ordering::Relaxed);
    result
}

/// The first lyrics found among the lookups' results; otherwise an error if any lookup failed, otherwise `None`.
fn first_found(results: impl Iterator<Item = Result<Option<Lyrics>>>) -> Result<Option<Lyrics>> {
    let mut failure = None;
    for result in results {
        match result {
            Ok(Some(lyrics)) => return Ok(Some(lyrics)),
            Ok(None) => {}
            Err(error) => failure = Some(error),
        }
    }
    failure.map_or(Ok(None), Err)
}

/// GET with retries: `Ok(None)` on 404, `Ok(Some(body))` on 200, and `Err` once every attempt failed (the service reports 503 when overloaded).
/// Retrying stops as soon as `is_settled` is set, because the other lookup already answered.
fn request(client: &HttpClient, url: &str, is_settled: &AtomicBool) -> Result<Option<HSTRING>> {
    let uri = Uri::CreateUri(&HSTRING::from(url))?;
    let mut last_error = windows::core::Error::from_hresult(windows::core::HRESULT(-1));
    for attempt in 0..=RETRY_DELAYS.len() {
        if attempt > 0 {
            thread::sleep(RETRY_DELAYS[attempt - 1]);
        }
        if is_settled.load(Ordering::Relaxed) {
            break;
        }
        let response = match client.GetAsync(&uri).and_then(|operation| operation.join()) {
            Ok(response) => response,
            Err(error) => {
                last_error = error;
                continue;
            }
        };
        match response.StatusCode()? {
            HttpStatusCode::Ok => return Ok(Some(response.Content()?.ReadAsStringAsync()?.join()?)),
            HttpStatusCode::NotFound => return Ok(None),
            status => last_error = windows::core::Error::from_hresult(windows::core::HRESULT(status.0)),
        }
    }
    Err(last_error)
}

fn lyrics_from_object(object: &JsonObject) -> Option<Lyrics> {
    let synced = object.GetNamedString(h!("syncedLyrics")).ok()?.to_string();
    let lines = parse_synced_lyrics(&synced);
    (!lines.is_empty()).then_some(Lyrics { lines })
}

/// Picks the search result whose length is closest to the playing track, ignoring results more than a few seconds off.
fn best_search_match(body: &HSTRING, duration_seconds: i64) -> Option<Lyrics> {
    let results = JsonArray::Parse(body).ok()?;
    let mut best: Option<(f64, Lyrics)> = None;
    for index in 0..results.Size().ok()? {
        let Ok(object) = results.GetObjectAt(index) else { continue };
        let Some(lyrics) = lyrics_from_object(&object) else { continue };
        let difference = if duration_seconds > 0 {
            (object.GetNamedNumber(h!("duration")).unwrap_or(0.0) - duration_seconds as f64).abs()
        } else {
            0.0
        };
        if difference > SEARCH_DURATION_TOLERANCE_SECONDS
            || best.as_ref().is_some_and(|(closest, _)| *closest <= difference)
        {
            continue;
        }
        best = Some((difference, lyrics));
    }
    best.map(|(_, lyrics)| lyrics)
}

fn clean_title(title: &str) -> String {
    let cut = [" (", " [", " - ", " feat", " ft."]
        .iter()
        .filter_map(|marker| title.find(marker))
        .min()
        .unwrap_or(title.len());
    title[..cut].trim().to_owned()
}

fn first_artist(artist: &str) -> &str {
    let cut = artist.find([',', '&', ';']).unwrap_or(artist.len());
    artist[..cut].trim()
}

fn percent_encode(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => encoded.push(byte as char),
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_encoding_escapes_everything_but_unreserved_characters() {
        assert_eq!(percent_encode("Ellie Goulding"), "Ellie%20Goulding");
        assert_eq!(percent_encode("a-b_c.d~e"), "a-b_c.d~e");
        assert_eq!(percent_encode("a&b=c"), "a%26b%3Dc");
        assert_eq!(percent_encode("caf\u{e9}"), "caf%C3%A9");
        assert_eq!(percent_encode(""), "");
    }

    #[test]
    fn titles_lose_suffixes_the_lyric_database_lacks() {
        assert_eq!(clean_title("Best Part (feat. H.E.R.)"), "Best Part");
        assert_eq!(clean_title("Song - Remastered 2011"), "Song");
        assert_eq!(clean_title("Title [Live]"), "Title");
        assert_eq!(clean_title("Dance feat. Someone"), "Dance");
        assert_eq!(clean_title("Plain Title"), "Plain Title");
    }

    fn found(text: &str) -> Result<Option<Lyrics>> {
        Ok(Some(Lyrics { lines: parse_synced_lyrics(text) }))
    }

    #[test]
    fn the_first_lookup_that_finds_lyrics_wins_even_after_a_failure() {
        let failed = Err(windows::core::Error::from_hresult(windows::core::HRESULT(-1)));
        let result = first_found([failed, found("[00:01.00] a")].into_iter());
        assert!(matches!(result, Ok(Some(lyrics)) if lyrics.lines.len() == 1));
    }

    #[test]
    fn no_lyrics_anywhere_is_none_but_a_failure_without_a_find_is_an_error() {
        assert!(matches!(first_found([Ok(None), Ok(None)].into_iter()), Ok(None)));
        let failed = Err(windows::core::Error::from_hresult(windows::core::HRESULT(-1)));
        assert!(first_found([Ok(None), failed].into_iter()).is_err());
    }

    #[test]
    fn only_the_first_artist_is_searched() {
        assert_eq!(first_artist("Daniel Caesar, H.E.R."), "Daniel Caesar");
        assert_eq!(first_artist("A & B"), "A");
        assert_eq!(first_artist("One; Two"), "One");
        assert_eq!(first_artist("Solo Artist"), "Solo Artist");
    }
}
