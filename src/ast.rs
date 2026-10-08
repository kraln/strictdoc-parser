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
    /// `PREFIX:` or its older spelling `REQ_PREFIX:`.
    pub prefix: Option<String>,
    /// Indented `OPTIONS:` sub-block.
    pub options: BTreeMap<String, String>,
    /// Indented `METADATA:` sub-block (free-form `key: value` pairs).
    pub metadata: BTreeMap<String, String>,
    /// Path from `[GRAMMAR]` `IMPORT_FROM_FILE:` if present. The referenced
    /// grammar is **not** loaded or validated by this parser.
    pub grammar_import: Option<String>,
    /// Top-level sections, nodes and includes in source order.
    pub body: Vec<DocumentChild>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum DocumentChild {
    /// `[[SECTION]]` … `[[/SECTION]]` (or the legacy `[SECTION]` …
    /// `[/SECTION]` form).
    Section(Section),
    /// Any other element: `[REQUIREMENT]`, `[TEXT]`, a custom-grammar
    /// element such as `[FEATURE]`, or a composite `[[TAG]]` … `[[/TAG]]`.
    Node(Node),
    /// `[DOCUMENT_FROM_FILE]` include. The referenced file is not loaded.
    DocumentFromFile(DocumentFromFile),
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Section {
    /// Value of the section's `TITLE:` field.
    pub title: String,
    /// All header fields (`TITLE`, `UID`, `LEVEL`, `PREFIX`, …) in source
    /// order.
    pub fields: Vec<Field>,
    pub children: Vec<DocumentChild>,
    pub span: Span,
}

impl Section {
    /// Return the first field with the given name (case-sensitive), if any.
    pub fn field(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// Text of the first field with `name`, if any.
    pub fn field_text(&self, name: &str) -> Option<&str> {
        self.field(name).map(|f| f.value.text())
    }
}

/// A StrictDoc node: one `[TAG]` block, or a composite `[[TAG]]` …
/// `[[/TAG]]` block with nested children.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Node {
    /// The element tag, e.g. `"REQUIREMENT"`, `"TEXT"`, `"FEATURE"`.
    pub node_type: String,
    /// Fields in source order, excluding `RELATIONS:`. Names are preserved
    /// verbatim (e.g. `"UID"`, `"STATEMENT"`).
    pub fields: Vec<Field>,
    /// Entries of the `RELATIONS:` list, in source order.
    pub relations: Vec<Relation>,
    /// True for `[[TAG]]` … `[[/TAG]]` composite nodes.
    pub composite: bool,
    /// Children of a composite node (always empty for `[TAG]` nodes).
    pub children: Vec<DocumentChild>,
    pub span: Span,
}

/// Former name of [`Node`]; kept so 0.1 code keeps compiling.
#[deprecated(since = "0.2.0", note = "renamed to `Node`")]
pub type Requirement = Node;

impl Node {
    /// Return the first field with the given name (case-sensitive), if any.
    pub fn field(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// Convenience: text of the first field with `name`, regardless of
    /// single-line vs heredoc.
    pub fn field_text(&self, name: &str) -> Option<&str> {
        self.field(name).map(|f| f.value.text())
    }

    /// True unless this is a `[TEXT]` node. Mirrors upstream StrictDoc's
    /// notion of a normative node (sections are not nodes here).
    pub fn is_normative(&self) -> bool {
        self.node_type != "TEXT"
    }
}

/// One entry of a node's `RELATIONS:` list, e.g.
///
/// ```text
/// - TYPE: Parent
///   VALUE: REQ-001
///   ROLE: Refines
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Relation {
    /// Value of `TYPE:` — `Parent`, `Child` or `File` in upstream StrictDoc.
    pub relation_type: String,
    /// Value of `ROLE:`, if present.
    pub role: Option<String>,
    /// Every other `KEY: value` property (`VALUE`, `PATH`, `LINE_RANGE`,
    /// …) in source order.
    pub properties: Vec<(String, String)>,
    pub span: Span,
}

impl Relation {
    /// Value of the first property named `key`, if any.
    pub fn property(&self, key: &str) -> Option<&str> {
        self.properties
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// The relation target: the UID for `Parent`/`Child` relations, the
    /// path for `File` relations (`PATH:`, or the deprecated `VALUE:`).
    pub fn target(&self) -> Option<&str> {
        if self.relation_type == "File" {
            self.property("PATH").or_else(|| self.property("VALUE"))
        } else {
            self.property("VALUE")
        }
    }
}

/// A `[DOCUMENT_FROM_FILE]` include.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DocumentFromFile {
    /// Value of the `FILE:` field, verbatim.
    pub file: String,
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
    /// `KEY: value` on a single line. `text` excludes the trailing newline
    /// and surrounding whitespace.
    SingleLine { text: String, span: Span },
    /// `KEY: >>>\n...\n<<<` heredoc. `text` is every line between the
    /// markers, verbatim, joined with `\n` (no trailing newline).
    Heredoc { text: String, span: Span },
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
