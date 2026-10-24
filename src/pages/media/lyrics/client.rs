//! Looks up synced lyrics on LRCLIB (https://lrclib.net), a free lyrics database that needs no account.

use std::{thread, time::Duration};

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
    if let Some(body) = request(&client, &exact_url)?
        && let Some(lyrics) = JsonObject::Parse(&body).ok().and_then(|object| lyrics_from_object(&object))
    {
        return Ok(Some(lyrics));
    }

    // Player titles often carry suffixes the lyric database lacks ("(feat. X)", "- Remastered"), so search by the plain name.
    let search_url = format!(
        "https://lrclib.net/api/search?track_name={}&artist_name={}",
        percent_encode(&clean_title(title)),
        percent_encode(first_artist(artist)),
    );
    Ok(request(&client, &search_url)?.and_then(|body| best_search_match(&body, duration_seconds)))
}

/// GET with retries: `Ok(None)` on 404, `Ok(Some(body))` on 200, and `Err` once every attempt failed (the service reports 503 when overloaded).
fn request(client: &HttpClient, url: &str) -> Result<Option<HSTRING>> {
    let uri = Uri::CreateUri(&HSTRING::from(url))?;
    let mut last_error = windows::core::Error::from_hresult(windows::core::HRESULT(-1));
    for attempt in 0..=RETRY_DELAYS.len() {
        if attempt > 0 {
            thread::sleep(RETRY_DELAYS[attempt - 1]);
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

    #[test]
    fn only_the_first_artist_is_searched() {
        assert_eq!(first_artist("Daniel Caesar, H.E.R."), "Daniel Caesar");
        assert_eq!(first_artist("A & B"), "A");
        assert_eq!(first_artist("One; Two"), "One");
        assert_eq!(first_artist("Solo Artist"), "Solo Artist");
    }
}
