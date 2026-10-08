// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

#![doc = include_str!("../README.md")]

pub mod annotation;
pub mod ast;
pub mod error;

mod lexer;
mod parser;
mod req_def;

pub use annotation::{
    find_relation_annotations, parse_relation_annotation, AnnotationError, AnnotationErrorKind,
    RelationAnnotation, RelationRole, RelationScope,
};
#[allow(deprecated)]
pub use ast::Requirement;
pub use ast::{
    Document, DocumentChild, DocumentFromFile, Field, FieldValue, Node, Relation, Section, Span,
};
pub use error::{ParseError, ParseErrorKind};
pub use req_def::RequirementView;

/// Parse a complete `.sdoc` file into a [`Document`].
///
/// The parser is non-recovering: the first syntax error stops parsing.
pub fn parse(input: &str) -> Result<Document, ParseError> {
    parser::parse_document(input)
}
