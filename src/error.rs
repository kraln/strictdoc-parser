// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Parse error type.

use std::fmt;

/// A fatal parse error.
///
/// The parser does not perform recovery: parsing aborts on the first error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub kind: ParseErrorKind,
    /// 1-indexed line number.
    pub line: u32,
    /// 1-indexed column number.
    pub column: u32,
    /// 0-indexed byte offset into the input.
    pub offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParseErrorKind {
    /// A heredoc was opened with `>>>` but never closed with `<<<` at the
    /// start of a line.
    UnterminatedHeredoc,
    /// Encountered a closing tag (`[[/TAG]]` or `[/TAG]`) with no open block.
    UnmatchedClose { tag: String },
    /// A closing tag did not match the innermost open block.
    MismatchedClose { expected: String, found: String },
    /// EOF reached with a block (`[[SECTION]]`, `[[TAG]]`, …) still open.
    UnclosedBlock { tag: String },
    /// A section was missing its `TITLE:` field.
    MissingSectionTitle,
    /// A `RELATIONS:` entry did not start with `- TYPE: …`.
    MalformedRelation,
    /// Encountered content that is not part of any block, or a line inside
    /// a block that is neither a field nor part of a heredoc.
    UnexpectedContent,
    /// The document started with something other than `[DOCUMENT]`.
    MissingDocumentBlock,
}

impl ParseError {
    // r[impl err.line-col]
    // r[impl err.fatal]
    pub(crate) fn new(kind: ParseErrorKind, line: u32, column: u32, offset: usize) -> Self {
        Self {
            kind,
            line,
            column,
            offset,
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "parse error at line {}, column {}: ",
            self.line, self.column
        )?;
        match &self.kind {
            ParseErrorKind::UnterminatedHeredoc => {
                f.write_str("unterminated heredoc (`<<<` not found)")
            }
            ParseErrorKind::UnmatchedClose { tag } => {
                write!(f, "`{tag}` without a matching open")
            }
            ParseErrorKind::MismatchedClose { expected, found } => {
                write!(f, "expected `{expected}`, found `{found}`")
            }
            ParseErrorKind::UnclosedBlock { tag } => {
                write!(f, "`{tag}` was opened but never closed")
            }
            ParseErrorKind::MissingSectionTitle => {
                f.write_str("section is missing its `TITLE:` field")
            }
            ParseErrorKind::MalformedRelation => {
                f.write_str("malformed relation (expected `- TYPE: ...`)")
            }
            ParseErrorKind::UnexpectedContent => f.write_str("unexpected content"),
            ParseErrorKind::MissingDocumentBlock => {
                f.write_str("expected `[DOCUMENT]` block at the top of the file")
            }
        }
    }
}

impl std::error::Error for ParseError {}
