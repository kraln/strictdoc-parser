// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! AST types for parsed StrictDoc documents.

use std::collections::BTreeMap;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A byte-offset span in the original input, with the source line and column
/// of the start.
///
/// Offsets are byte positions (0-indexed). `line` and `column` are 1-indexed
/// and refer to the start position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: u32,
    pub column: u32,
}

impl Span {
    // r[impl span.every-node]
    // r[impl span.utf8-correct]
    pub(crate) fn new(start: usize, end: usize, line: u32, column: u32) -> Self {
        Self {
            start,
            end,
            line,
            column,
        }
    }
}

/// A parsed StrictDoc document.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Document {
    pub title: Option<String>,
    pub uid: Option<String>,
    pub version: Option<String>,
    pub date: Option<String>,
    pub classification: Option<String>,
    pub prefix: Option<String>,
    /// Indented `OPTIONS:` sub-block.
    pub options: BTreeMap<String, String>,
    /// Path from `[GRAMMAR]` `IMPORT_FROM_FILE:` if present. The referenced
    /// grammar is **not** loaded or validated by this parser.
    pub grammar_import: Option<String>,
    /// Sections and top-level requirements in source order.
    pub body: Vec<DocumentChild>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum DocumentChild {
    Section(Section),
    Requirement(Requirement),
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Section {
    pub title: String,
    pub children: Vec<DocumentChild>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Requirement {
    /// Fields in source order. Names are preserved verbatim (e.g. `"UID"`,
    /// `"STATEMENT"`).
    pub fields: Vec<Field>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Field {
    pub name: String,
    pub value: FieldValue,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum FieldValue {
    /// `KEY: value` on a single line. `text` excludes the trailing newline.
    SingleLine { text: String, span: Span },
    /// `KEY: >>>\n...\n<<<` heredoc. `text` is the content between the
    /// markers, verbatim, with leading and trailing blank lines stripped.
    Heredoc { text: String, span: Span },
}

impl Requirement {
    /// Return the first field with the given name (case-sensitive), if any.
    pub fn field(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// Convenience: text of the first field with `name`, regardless of
    /// single-line vs heredoc.
    pub fn field_text(&self, name: &str) -> Option<&str> {
        self.field(name).map(|f| f.value.text())
    }
}

impl FieldValue {
    pub fn text(&self) -> &str {
        match self {
            FieldValue::SingleLine { text, .. } => text,
            FieldValue::Heredoc { text, .. } => text,
        }
    }

    pub fn span(&self) -> Span {
        match self {
            FieldValue::SingleLine { span, .. } => *span,
            FieldValue::Heredoc { span, .. } => *span,
        }
    }
}
