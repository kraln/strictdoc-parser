// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Parse error type.

use std::fmt;

/// A fatal parse error.
///
/// v0.1 does not perform recovery: parsing aborts on the first error.
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
pub enum ParseErrorKind {
    /// A heredoc was opened with `>>>` but never closed with `<<<`.
    UnterminatedHeredoc,
    /// Encountered `[[/SECTION]]` without a matching open.
    UnmatchedSectionClose,
    /// EOF reached with a section still open.
    UnclosedSection,
    /// A `[REQUIREMENT]` was missing a required field (e.g. UID).
    MissingRequirementField { name: String },
    /// A `[[SECTION]]` was missing its `TITLE:` field.
    MissingSectionTitle,
    /// Found an unrecognised block tag (e.g. `[BOGUS]`).
    UnknownBlock { tag: String },
    /// A `KEY: value` line had no `:` separator where one was expected.
    MalformedField,
    /// A `[[TAG]]` line had no matching `]]` close brace.
    MalformedBlockHeader,
    /// Encountered content that wasn't part of any block (stray text at the
    /// top level before `[DOCUMENT]` or between sections).
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
            ParseErrorKind::UnmatchedSectionClose => {
                f.write_str("`[[/SECTION]]` without a matching open")
            }
            ParseErrorKind::UnclosedSection => f.write_str("section was opened but never closed"),
            ParseErrorKind::MissingRequirementField { name } => {
                write!(f, "requirement is missing required field `{name}`")
            }
            ParseErrorKind::MissingSectionTitle => {
                f.write_str("section is missing its `TITLE:` field")
            }
            ParseErrorKind::UnknownBlock { tag } => write!(f, "unknown block tag `{tag}`"),
            ParseErrorKind::MalformedField => {
                f.write_str("malformed field (expected `KEY: value`)")
            }
            ParseErrorKind::MalformedBlockHeader => {
                f.write_str("malformed block header (missing closing brackets)")
            }
            ParseErrorKind::UnexpectedContent => {
                f.write_str("unexpected content outside any block")
            }
            ParseErrorKind::MissingDocumentBlock => {
                f.write_str("expected `[DOCUMENT]` block at the top of the file")
            }
        }
    }
}

impl std::error::Error for ParseError {}
