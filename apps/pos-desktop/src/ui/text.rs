//! Thai-safe line breaking (design/docs/01 Typography, "ตัดคำไทย").
//! egui breaks only at whitespace and otherwise between any two characters,
//! which can strand a leading vowel (ไ|ข่) or split a tone mark from its
//! consonant. Without a dictionary the system allows breaks at spaces and
//! `+ / ( )`; a run that still does not fit breaks between Thai character
//! clusters, never inside one.
use eframe::egui::{self, Color32, FontId, Galley};
use std::sync::Arc;

/// Combining marks and following vowels that belong to the previous character.
fn attaches_to_previous(c: char) -> bool {
    matches!(c, '\u{0E30}'..='\u{0E3A}' | '\u{0E45}' | '\u{0E47}'..='\u{0E4E}')
}
/// Leading vowels เ แ โ ใ ไ that belong to the next character.
fn leads_next(c: char) -> bool {
    matches!(c, '\u{0E40}'..='\u{0E44}')
}
/// Splits text into the smallest pieces a line may break between.
fn clusters(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut prev: Option<char> = None;
    for (i, c) in text.char_indices() {
        if let Some(p) = prev
            && !attaches_to_previous(c)
            && !leads_next(p)
        {
            out.push(&text[start..i]);
            start = i;
        }
        prev = Some(c);
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}
/// Preferred pieces: break after spaces, `/`, `+`, `)` and before `(`.
fn tokens(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        let next = chars.peek().map(|(_, n)| *n);
        let end = i + c.len_utf8();
        let break_after = c == ' ' || c == '/' || c == '+' || c == ')' || next == Some('(');
        if break_after && end < text.len() {
            out.push(&text[start..end]);
            start = end;
        }
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

/// Inserts line breaks so `text` fits `width`, keeping at most `max_rows`
/// rows (0 = unlimited) and ending an elided last row with `…`.
/// Returns the broken text and whether anything was cut.
pub fn break_lines(
    measure: &dyn Fn(&str) -> f32,
    text: &str,
    width: f32,
    max_rows: usize,
) -> (String, bool) {
    let mut lines: Vec<String> = vec![String::new()];
    let push_piece = |lines: &mut Vec<String>, piece: &str| {
        let Some(line) = lines.last_mut() else {
            return;
        };
        let candidate = format!("{line}{piece}");
        if line.is_empty() || measure(candidate.trim_end()) <= width {
            *line = candidate;
        } else {
            lines.push(piece.trim_start().to_owned());
        }
    };
    for token in tokens(text) {
        let current = lines.last().map_or("", String::as_str);
        let fits_alone = measure(token.trim()) <= width;
        let candidate = format!("{current}{token}");
        if current.is_empty() && fits_alone || measure(candidate.trim_end()) <= width {
            if let Some(line) = lines.last_mut() {
                *line = candidate;
            }
        } else if fits_alone {
            lines.push(token.trim_start().to_owned());
        } else {
            for cluster in clusters(token) {
                push_piece(&mut lines, cluster);
            }
        }
    }
    let mut cut = false;
    if max_rows > 0 && lines.len() > max_rows {
        lines.truncate(max_rows);
        cut = true;
        if let Some(last) = lines.last_mut() {
            let mut kept: Vec<&str> = clusters(last.trim_end());
            while !kept.is_empty() && measure(&format!("{}…", kept.concat())) > width {
                kept.pop();
            }
            *last = format!("{}…", kept.concat().trim_end());
        }
    }
    let lines: Vec<&str> = lines.iter().map(|l| l.trim_end()).collect();
    (lines.join("\n"), cut)
}

/// A galley laid out with Thai-safe breaks. `elided` reports truncation.
pub fn wrapped(
    painter: &egui::Painter,
    text: &str,
    font: FontId,
    color: Color32,
    width: f32,
    max_rows: usize,
) -> (Arc<Galley>, bool) {
    let measure = |s: &str| {
        painter
            .layout_no_wrap(s.to_owned(), font.clone(), color)
            .size()
            .x
    };
    let (broken, cut) = break_lines(&measure, text, width.max(1.0), max_rows);
    (painter.layout_no_wrap(broken, font, color), cut)
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Every character is one unit wide.
    fn units(s: &str) -> f32 {
        s.chars().count() as f32
    }
    #[test]
    fn leading_vowels_and_marks_stay_with_their_consonant() {
        assert_eq!(clusters("ไข่ดาว"), ["ไข่", "ดา", "ว"]);
        assert_eq!(clusters("น้ำ"), ["น้ำ"]);
        assert_eq!(clusters("ที่นี่"), ["ที่", "นี่"]);
    }
    #[test]
    fn breaks_prefer_spaces_and_slashes() {
        let (text, cut) = break_lines(&units, "ข้าวมันไก่ต้ม / Hainanese chicken rice", 16.0, 0);
        assert!(!cut);
        for line in text.lines() {
            assert!(units(line) <= 16.0, "{line}");
        }
        assert!(text.starts_with("ข้าวมันไก่ต้ม /"));
    }
    #[test]
    fn long_thai_runs_never_strand_a_leading_vowel() {
        let (text, _) = break_lines(&units, "ข้าวกะเพราไก่ไข่ดาว", 9.0, 0);
        for line in text.lines() {
            let last = line.chars().last().unwrap_or(' ');
            assert!(!leads_next(last), "line ends with a leading vowel: {line}");
            let first = line.chars().next().unwrap_or(' ');
            assert!(
                !attaches_to_previous(first),
                "line starts with a mark: {line}"
            );
        }
    }
    #[test]
    fn rows_are_limited_with_an_ellipsis() {
        let (text, cut) = break_lines(&units, "one two three four five six", 9.0, 2);
        assert!(cut);
        assert_eq!(text.lines().count(), 2);
        assert!(text.ends_with('…'));
    }
}
