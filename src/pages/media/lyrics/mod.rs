//! Time-synced lyrics: the data model and the parser for the `.lrc` text format.

mod client;

pub use client::fetch;

const TICKS_PER_SECOND: f64 = 10_000_000.0;

pub struct LyricLine {
    pub start_ticks: i64,
    pub text: String,
}

/// Time-synced lyrics, sorted by start time.
pub struct Lyrics {
    pub lines: Vec<LyricLine>,
}

impl Lyrics {
    pub fn index_at(&self, position_ticks: i64) -> Option<usize> {
        self.lines.partition_point(|line| line.start_ticks <= position_ticks).checked_sub(1)
    }

    /// Start time of the line after the one at `index`; `None` index means the first line.
    pub fn next_start_after(&self, index: Option<usize>) -> Option<i64> {
        let next_index = index.map_or(0, |current| current + 1);
        self.lines.get(next_index).map(|line| line.start_ticks)
    }
}

/// Parses LRC text: lines like `[01:23.45] words`, where one line may carry several timestamps.
fn parse_synced_lyrics(text: &str) -> Vec<LyricLine> {
    let mut lines = Vec::new();
    for raw_line in text.lines() {
        let mut rest = raw_line.trim();
        let mut starts = Vec::new();
        while let Some(stripped) = rest.strip_prefix('[') {
            let Some((tag, after)) = stripped.split_once(']') else { break };
            if let Some(ticks) = parse_timestamp(tag) {
                starts.push(ticks);
            }
            rest = after;
        }
        let words = rest.trim();
        lines.extend(starts.into_iter().map(|start_ticks| LyricLine { start_ticks, text: words.to_owned() }));
    }
    lines.sort_by_key(|line| line.start_ticks);
    lines
}

fn parse_timestamp(tag: &str) -> Option<i64> {
    let (minutes, seconds) = tag.split_once(':')?;
    let minutes: f64 = minutes.parse().ok()?;
    let seconds: f64 = seconds.parse().ok()?;
    Some(((minutes * 60.0 + seconds) * TICKS_PER_SECOND) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECOND: i64 = 10_000_000;

    fn lyrics_starting_at(seconds: &[i64]) -> Lyrics {
        Lyrics {
            lines: seconds
                .iter()
                .map(|&second| LyricLine { start_ticks: second * SECOND, text: format!("line {second}") })
                .collect(),
        }
    }

    #[test]
    fn parses_timestamps_and_text() {
        let lines = parse_synced_lyrics("[00:05.50] hello\n[01:02.00] world");
        assert_eq!(lines.len(), 2);
        assert_eq!((lines[0].start_ticks, lines[0].text.as_str()), (55_000_000, "hello"));
        assert_eq!((lines[1].start_ticks, lines[1].text.as_str()), (620_000_000, "world"));
    }

    #[test]
    fn a_line_with_several_timestamps_repeats_for_each() {
        let lines = parse_synced_lyrics("[00:30.00][00:10.00] chorus");
        let starts: Vec<i64> = lines.iter().map(|line| line.start_ticks).collect();
        assert_eq!(starts, vec![10 * SECOND, 30 * SECOND]);
        assert!(lines.iter().all(|line| line.text == "chorus"));
    }

    #[test]
    fn metadata_tags_are_not_lyrics() {
        let lines = parse_synced_lyrics("[ar:Someone]\n[ti:A Song]\n[00:01.00] first");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "first");
    }

    #[test]
    fn empty_lines_are_kept_for_instrumental_gaps() {
        let lines = parse_synced_lyrics("[00:01.00] a\n[00:05.00]\n[00:09.00] b");
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[1].text, "");
    }

    #[test]
    fn lines_are_sorted_by_start_time() {
        let lines = parse_synced_lyrics("[00:09.00] late\n[00:01.00] early");
        assert_eq!(lines[0].text, "early");
    }

    #[test]
    fn text_without_timestamps_has_no_lines() {
        assert!(parse_synced_lyrics("just words\nand more words").is_empty());
        assert!(parse_synced_lyrics("").is_empty());
    }

    #[test]
    fn finds_the_line_being_sung() {
        let lyrics = lyrics_starting_at(&[10, 20, 30]);
        assert_eq!(lyrics.index_at(0), None);
        assert_eq!(lyrics.index_at(10 * SECOND - 1), None);
        assert_eq!(lyrics.index_at(10 * SECOND), Some(0));
        assert_eq!(lyrics.index_at(25 * SECOND), Some(1));
        assert_eq!(lyrics.index_at(999 * SECOND), Some(2));
    }

    #[test]
    fn finds_when_the_next_line_starts() {
        let lyrics = lyrics_starting_at(&[10, 20]);
        assert_eq!(lyrics.next_start_after(None), Some(10 * SECOND));
        assert_eq!(lyrics.next_start_after(Some(0)), Some(20 * SECOND));
        assert_eq!(lyrics.next_start_after(Some(1)), None);
    }
}
