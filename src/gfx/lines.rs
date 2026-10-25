//! Breaks text into a limited number of lines that fit a width, like CSS `line-clamp`.

const ELLIPSIS: char = '\u{2026}';

/// Breaks `text` into at most `max_lines` lines no wider than `max_width` (as `measure` reports). The words wrap; a word
/// wider than a line is cut where it stops fitting. When the text needs more lines, the last one ends with an ellipsis.
pub fn clamp_lines(text: &str, max_lines: usize, max_width: f32, measure: impl Fn(&str) -> f32) -> Vec<String> {
    clamp(text, max_lines, max_width, &measure)
}

/// Like `clamp_lines`, but when the text takes more than one line it makes the lines about equally long, like CSS
/// `text-wrap: balance`, by finding the narrowest width that still gives the same number of lines.
pub fn balance_lines(text: &str, max_lines: usize, max_width: f32, measure: impl Fn(&str) -> f32) -> Vec<String> {
    let lines = clamp(text, max_lines, max_width, &measure);
    let is_cut = lines.last().is_some_and(|last| last.ends_with(ELLIPSIS));
    if lines.len() < 2 || is_cut {
        return lines;
    }
    let (mut narrow, mut wide) = (0.0, max_width);
    for _ in 0..12 {
        let middle = (narrow + wide) / 2.0;
        let attempt = clamp(text, max_lines, middle, &measure);
        let fits = attempt.len() == lines.len() && !attempt.last().is_some_and(|last| last.ends_with(ELLIPSIS));
        if fits {
            wide = middle;
        } else {
            narrow = middle;
        }
    }
    clamp(text, max_lines, wide, &measure)
}

/// `clamp_lines` with the measuring function behind a pointer, so it can call itself without the type growing.
fn clamp(text: &str, max_lines: usize, max_width: f32, measure: &dyn Fn(&str) -> f32) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() || max_lines == 0 {
        return Vec::new();
    }
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut index = 0;
    while index < words.len() {
        let word = words[index];
        let candidate = if current.is_empty() { word.to_owned() } else { format!("{current} {word}") };
        if measure(&candidate) <= max_width {
            current = candidate;
            index += 1;
            continue;
        }
        if current.is_empty() {
            // One word wider than a whole line: take what fits and carry on with the rest.
            let (fitting, rest) = split_to_fit(word, max_width, measure);
            lines.push(fitting);
            if lines.len() == max_lines {
                return finish(lines, rest, &words[index + 1..], max_width, measure);
            }
            // The rest of the word is tried again as the next word.
            return clamp_rest(lines, rest, &words[index + 1..], max_lines, max_width, measure);
        }
        lines.push(std::mem::take(&mut current));
        if lines.len() == max_lines {
            return finish(lines, "", &words[index..], max_width, measure);
        }
    }
    lines.push(current);
    lines
}

/// Carries on after a long word was cut: lays out its remainder followed by the words that were left.
fn clamp_rest(
    mut lines: Vec<String>,
    rest: &str,
    words: &[&str],
    max_lines: usize,
    max_width: f32,
    measure: &dyn Fn(&str) -> f32,
) -> Vec<String> {
    let remaining = std::iter::once(rest).filter(|part| !part.is_empty()).chain(words.iter().copied());
    let tail = remaining.collect::<Vec<_>>().join(" ");
    lines.extend(clamp(&tail, max_lines - lines.len(), max_width, measure));
    lines
}

/// Replaces the last line with as much of it and of the text that follows as fits, ending in an ellipsis; when
/// nothing follows, the last line stays as it is.
fn finish(
    mut lines: Vec<String>,
    rest_of_word: &str,
    following: &[&str],
    max_width: f32,
    measure: &dyn Fn(&str) -> f32,
) -> Vec<String> {
    let overflow = std::iter::once(rest_of_word).filter(|part| !part.is_empty()).chain(following.iter().copied());
    let overflow = overflow.collect::<Vec<_>>().join(" ");
    if overflow.is_empty() {
        return lines;
    }
    let last = lines.pop().unwrap_or_default();
    let full = format!("{last} {overflow}");
    lines.push(with_ellipsis(&full, max_width, measure));
    lines
}

/// The longest start of `text` that, with an ellipsis after it, fits `max_width`.
fn with_ellipsis(text: &str, max_width: f32, measure: &dyn Fn(&str) -> f32) -> String {
    let mut end = text.len();
    loop {
        let candidate = format!("{}{ELLIPSIS}", text[..end].trim_end());
        if end == 0 || measure(&candidate) <= max_width {
            return candidate;
        }
        end = text[..end].char_indices().next_back().map_or(0, |(position, _)| position);
    }
}

/// Splits `word` into the longest start that fits `max_width` (at least one character) and the rest.
fn split_to_fit<'a>(word: &'a str, max_width: f32, measure: &dyn Fn(&str) -> f32) -> (String, &'a str) {
    let mut end = 0;
    for (position, character) in word.char_indices() {
        let next = position + character.len_utf8();
        if end > 0 && measure(&word[..next]) > max_width {
            break;
        }
        end = next;
    }
    (word[..end].to_owned(), &word[end..])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every character is one unit wide.
    fn width(text: &str) -> f32 {
        text.chars().count() as f32
    }

    #[test]
    fn short_text_stays_on_one_line() {
        assert_eq!(clamp_lines("Hello world", 2, 20.0, width), vec!["Hello world"]);
    }

    #[test]
    fn words_wrap_onto_the_next_line() {
        assert_eq!(clamp_lines("The Eliminating Angel", 2, 16.0, width), vec!["The Eliminating", "Angel"]);
    }

    #[test]
    fn text_that_needs_a_third_line_ends_the_second_with_an_ellipsis() {
        let lines = clamp_lines("one two three four five six", 2, 9.0, width);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "one two");
        assert!(lines[1].ends_with('\u{2026}'));
        assert!(width(&lines[1]) <= 9.0);
    }

    #[test]
    fn text_that_fits_exactly_in_the_allowed_lines_gets_no_ellipsis() {
        assert_eq!(clamp_lines("aaaa bbbb", 2, 4.0, width), vec!["aaaa", "bbbb"]);
    }

    #[test]
    fn a_word_wider_than_a_line_is_cut_and_continues_on_the_next_line() {
        assert_eq!(clamp_lines("abcdefghij", 3, 4.0, width), vec!["abcd", "efgh", "ij"]);
    }

    #[test]
    fn a_very_long_word_is_cut_with_an_ellipsis_on_the_last_line() {
        let lines = clamp_lines("abcdefghijklmnop", 2, 4.0, width);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "abcd");
        assert!(lines[1].ends_with('\u{2026}') && width(&lines[1]) <= 4.0);
    }

    #[test]
    fn no_text_or_no_lines_gives_no_lines() {
        assert!(clamp_lines("   ", 2, 10.0, width).is_empty());
        assert!(clamp_lines("text", 0, 10.0, width).is_empty());
    }

    #[test]
    fn balanced_lines_are_about_equally_long() {
        let text = "aaa bbb ccc ddd eee fff";
        assert_eq!(clamp_lines(text, 2, 20.0, width), vec!["aaa bbb ccc ddd eee", "fff"]);
        assert_eq!(balance_lines(text, 2, 20.0, width), vec!["aaa bbb ccc", "ddd eee fff"]);
    }

    #[test]
    fn balancing_leaves_one_line_and_cut_text_alone() {
        assert_eq!(balance_lines("short", 2, 20.0, width), vec!["short"]);
        let cut = balance_lines("one two three four five six seven eight", 2, 9.0, width);
        assert_eq!(cut, clamp_lines("one two three four five six seven eight", 2, 9.0, width));
    }

    #[test]
    fn repeated_spaces_collapse() {
        assert_eq!(clamp_lines("a   b", 1, 10.0, width), vec!["a b"]);
    }
}
