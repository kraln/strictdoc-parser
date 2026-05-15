// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Parser for [StrictDoc](https://strictdoc.readthedocs.io/) `.sdoc` files
//! and StrictDoc-style `@relation(...)` source-comment annotations.
//!
//! See the crate-level README for usage examples.

pub mod annotation;
pub mod ast;
pub mod error;

mod lexer;
mod parser;
mod req_def;

pub use annotation::{parse_relation_annotation, RelationAnnotation, RelationRole, RelationScope};
pub use ast::{Document, DocumentChild, Field, FieldValue, Requirement, Section, Span};
pub use error::{ParseError, ParseErrorKind};
pub use req_def::RequirementView;

/// Parse a complete `.sdoc` file into a [`Document`].
///
/// v0.1 parsers are non-recovering: the first syntax error stops parsing.
pub fn parse(input: &str) -> Result<Document, ParseError> {
    parser::parse_document(input)
}
