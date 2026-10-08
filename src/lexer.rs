// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Line-oriented tokenisation utilities.
//!
//! The StrictDoc text grammar is line-oriented (blocks open on `[TAG]` lines,
//! fields are `KEY: value` lines, heredocs span multiple lines between `>>>`
//! and `<<<`). The lexer here exposes an iterator over lines with byte-offset
//! spans, plus classification helpers the parser uses to decide what each
//! line represents.

/// A single line from the input, sans the trailing `\n`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Line<'a> {
    pub text: &'a str,
    pub start: usize,
    pub end: usize,
    pub line_no: u32,
}

/// Iterate over the lines of `input`, tracking byte offsets and 1-indexed
/// line numbers. The yielded `text` excludes the trailing `\n` (and `\r` if
/// present, treating `\r\n` as one line terminator).
#[cfg(test)]
pub(crate) fn lines(input: &str) -> Lines<'_> {
    lines_from(input, 0)
}

/// Like [`lines`], but start at byte offset `start` (which must be on a
/// char boundary and before the first line break), e.g. to skip a BOM.
pub(crate) fn lines_from(input: &str, start: usize) -> Lines<'_> {
    Lines {
        input,
        pos: start,
        line_no: 1,
    }
}

pub(crate) struct Lines<'a> {
    input: &'a str,
    pos: usize,
    line_no: u32,
}

impl<'a> Iterator for Lines<'a> {
    type Item = Line<'a>;

    fn next(&mut self) -> Option<Line<'a>> {
        if self.pos >= self.input.len() {
            return None;
        }
        let start = self.pos;
        // Find next '\n'.
        let bytes = &self.input.as_bytes()[start..];
        let nl_offset = bytes.iter().position(|&b| b == b'\n');
        let (text_end, next_pos) = match nl_offset {
            Some(o) => {
                let mut t_end = start + o;
                // Strip trailing \r if present.
                if t_end > start && self.input.as_bytes()[t_end - 1] == b'\r' {
                    t_end -= 1;
                }
                (t_end, start + o + 1)
            }
            None => (self.input.len(), self.input.len()),
        };
        let text = &self.input[start..text_end];
        let line = Line {
            text,
            start,
            end: text_end,
            line_no: self.line_no,
        };
        self.pos = next_pos;
        self.line_no += 1;
        Some(line)
    }
}

/// What kind of meaningful content a non-blank line carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LineKind<'a> {
    /// `[TAG]` (single brackets).
    BlockHeader { tag: &'a str },
    /// `[[TAG]]` (double brackets).
    DoubleBlockHeader { tag: &'a str },
    /// `[[/TAG]]` (double brackets, closer).
    BlockClose { tag: &'a str },
    /// `[/TAG]` (single brackets, closer) — only used by the legacy
    /// `[SECTION]` … `[/SECTION]` form.
    LegacyClose { tag: &'a str },
    /// `KEY: >>>` — heredoc opener for the field `KEY`.
    HeredocOpen { name: &'a str },
    /// `<<<` at the start of a line — heredoc closer.
    HeredocClose,
    /// `KEY: value` — single-line field with a non-empty value (and value
    /// is not exactly `>>>`).
    Field { name: &'a str, value: &'a str },
    /// `KEY:` with no value on the same line — indicates either an indented
    /// sub-block follows (e.g. `OPTIONS:`) or an empty value. The parser
    /// disambiguates based on whether the next line is indented.
    EmptyField { name: &'a str },
    /// An indented `KEY: value` line (one or more leading spaces).
    IndentedField {
        indent: usize,
        name: &'a str,
        value: &'a str,
    },
    /// Anything else: bare text not matching any of the above. The parser
    /// treats this as either heredoc content (if we're inside a heredoc) or
    /// an error (if we're at the document/block level).
    Other,
}

/// Classify a single line.
///
/// Block markers and the heredoc closer are only recognised at column 1,
/// as in upstream StrictDoc: an indented `<<<` or `[[SECTION]]` (e.g.
/// inside a code example in a heredoc) is ordinary text.
pub(crate) fn classify<'a>(line: &Line<'a>) -> LineKind<'a> {
    let trimmed = line.text.trim_end();
    let raw_trimmed = trimmed.trim_start();
    let indent = trimmed.len() - raw_trimmed.len();

    if indent == 0 {
        // r[impl req.heredoc-close]
        if trimmed == "<<<" {
            return LineKind::HeredocClose;
        }

        if let Some(inner) = trimmed
            .strip_prefix("[[")
            .and_then(|r| r.strip_suffix("]]"))
        {
            if let Some(tag) = inner.strip_prefix('/') {
                if is_tag(tag) {
                    return LineKind::BlockClose { tag };
                }
            } else if is_tag(inner) {
                return LineKind::DoubleBlockHeader { tag: inner };
            }
        } else if let Some(inner) = trimmed.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
            if let Some(tag) = inner.strip_prefix('/') {
                if is_tag(tag) {
                    return LineKind::LegacyClose { tag };
                }
            } else if is_tag(inner) {
                return LineKind::BlockHeader { tag: inner };
            }
        }
    }

    // KEY: value or KEY: >>> or KEY: (empty)
    if let Some(colon_idx) = raw_trimmed.find(':') {
        let name = &raw_trimmed[..colon_idx];
        if is_field_name(name) {
            let after = raw_trimmed[colon_idx + 1..].trim();
            if after == ">>>" && indent == 0 {
                return LineKind::HeredocOpen { name };
            }
            if indent > 0 {
                return LineKind::IndentedField {
                    indent,
                    name,
                    value: after,
                };
            }
            if after.is_empty() {
                return LineKind::EmptyField { name };
            }
            return LineKind::Field { name, value: after };
        }
    }

    LineKind::Other
}

/// True if `s` looks like a StrictDoc element tag (`[A-Z][A-Z0-9_]*`).
fn is_tag(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_uppercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// True if `s` looks like a StrictDoc field name. Upstream's rule is
/// `[A-Z]+[A-Za-z0-9_\-]*`: an uppercase first letter, then letters,
/// digits, `_` or `-` (e.g. `CHECKED-BY`, `Verified_by`).
fn is_field_name(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_uppercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// True if a line is blank (empty or whitespace-only).
pub(crate) fn is_blank(line: &Line<'_>) -> bool {
    line.text.chars().all(|c| c.is_whitespace())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_yields_byte_offsets_and_line_numbers() {
        let input = "a\nbb\nccc\n";
        let collected: Vec<_> = lines(input).collect();
        assert_eq!(collected.len(), 3);
        assert_eq!(collected[0].text, "a");
        assert_eq!(collected[0].start, 0);
        assert_eq!(collected[0].end, 1);
        assert_eq!(collected[0].line_no, 1);
        assert_eq!(collected[1].text, "bb");
        assert_eq!(collected[1].start, 2);
        assert_eq!(collected[1].end, 4);
        assert_eq!(collected[1].line_no, 2);
        assert_eq!(collected[2].text, "ccc");
        assert_eq!(collected[2].start, 5);
        assert_eq!(collected[2].end, 8);
    }

    #[test]
    fn lines_handles_no_trailing_newline() {
        let collected: Vec<_> = lines("abc").collect();
        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].text, "abc");
    }

    #[test]
    fn lines_handles_crlf() {
        let collected: Vec<_> = lines("a\r\nb\r\n").collect();
        assert_eq!(collected.len(), 2);
        assert_eq!(collected[0].text, "a");
        assert_eq!(collected[1].text, "b");
    }

    #[test]
    fn lines_preserves_utf8_byte_offsets() {
        // 'μ' is two bytes in UTF-8.
        let input = "μBS\nfoo\n";
        let collected: Vec<_> = lines(input).collect();
        assert_eq!(collected[0].text, "μBS");
        assert_eq!(collected[0].start, 0);
        assert_eq!(collected[0].end, 4); // 2 bytes for μ + 1 each for B, S
        assert_eq!(collected[1].start, 5);
    }

    fn classify_str(s: &str) -> LineKind<'_> {
        let line = Line {
            text: s,
            start: 0,
            end: s.len(),
            line_no: 1,
        };
        classify(&line)
    }

    #[test]
    fn classify_block_headers() {
        assert!(matches!(
            classify_str("[DOCUMENT]"),
            LineKind::BlockHeader { tag: "DOCUMENT" }
        ));
        assert!(matches!(
            classify_str("[[SECTION]]"),
            LineKind::DoubleBlockHeader { tag: "SECTION" }
        ));
        assert!(matches!(
            classify_str("[[/SECTION]]"),
            LineKind::BlockClose { tag: "SECTION" }
        ));
    }

    #[test]
    fn classify_fields() {
        assert!(matches!(
            classify_str("UID: BR-001"),
            LineKind::Field {
                name: "UID",
                value: "BR-001"
            }
        ));
        assert!(matches!(
            classify_str("STATEMENT: >>>"),
            LineKind::HeredocOpen { name: "STATEMENT" }
        ));
        assert!(matches!(classify_str("<<<"), LineKind::HeredocClose));
        assert!(matches!(
            classify_str("OPTIONS:"),
            LineKind::EmptyField { name: "OPTIONS" }
        ));
        assert!(matches!(
            classify_str("  MARKUP: Text"),
            LineKind::IndentedField {
                indent: 2,
                name: "MARKUP",
                value: "Text"
            }
        ));
    }

    #[test]
    fn classify_markers_only_at_column_one() {
        assert!(matches!(classify_str("    <<<"), LineKind::Other));
        assert!(matches!(classify_str("<<<  "), LineKind::HeredocClose));
        assert!(matches!(classify_str("  [[SECTION]]"), LineKind::Other));
        assert!(matches!(
            classify_str("[/SECTION]"),
            LineKind::LegacyClose { tag: "SECTION" }
        ));
        assert!(matches!(classify_str("[LINK: X]"), LineKind::Other));
        assert!(matches!(
            classify_str("Verified_by: x"),
            LineKind::Field {
                name: "Verified_by",
                value: "x"
            }
        ));
    }

    #[test]
    fn classify_rejects_non_uppercase_field_names() {
        // Lowercase names are not StrictDoc fields.
        assert!(matches!(classify_str("lowercase: value"), LineKind::Other));
        assert!(matches!(classify_str("Two words: value"), LineKind::Other));
        // Sentence with a colon inside isn't a field either.
        assert!(matches!(classify_str("This is a note."), LineKind::Other));
    }
}
