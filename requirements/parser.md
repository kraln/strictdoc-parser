# strictdoc-parser — Requirements

This document is the tracey-readable specification for the
`strictdoc-parser` crate. Each `r[…]` marker defines a requirement covered
by the implementation and tests in this repository.

## Document structure

r[doc.header]
The parser MUST accept a single `[DOCUMENT]` header as the first non-blank
block in a `.sdoc` file and MUST reject input whose first block is anything
else.

r[doc.fields]
The parser MUST extract `TITLE`, `UID`, `VERSION`, `DATE`,
`CLASSIFICATION`, and `PREFIX` fields from the `[DOCUMENT]` header when
present, treating each as optional at the parser layer.

r[doc.options]
The parser MUST collect the indented sub-block following an `OPTIONS:`
header into a key-value map on the parsed `Document`.

r[doc.grammar-import]
The parser MUST recognise a `[GRAMMAR]` block and surface the path from
its `IMPORT_FROM_FILE:` field on the parsed `Document`. The parser MUST
NOT attempt to load or validate the referenced grammar file.

## Sections

r[sect.open-close]
The parser MUST pair `[[SECTION]]` opens with `[[/SECTION]]` closes and
MUST report a parse error with line and column information when a section
is left unclosed at EOF.

r[sect.title]
The parser MUST require sections to carry a `TITLE:` field and MUST report
a parse error when a section's closing marker is reached without one.

r[sect.nesting]
The parser MUST support `[[SECTION]]` blocks nested inside other
`[[SECTION]]` blocks to a depth of at least four levels.

## Requirements

r[req.fields]
The parser MUST parse single-line `KEY: value` fields and multi-line
`KEY: >>>` … `<<<` heredoc fields inside `[REQUIREMENT]` blocks, producing
a `FieldValue::SingleLine` or `FieldValue::Heredoc` respectively.

r[req.field-order]
The parser MUST preserve the source order of fields within each
requirement.

r[req.heredoc-verbatim]
The parser MUST preserve heredoc body content verbatim, including embedded
newlines and interior whitespace, stripping only fully blank lines at the
start and end of the body.

r[req.uid-verbatim]
The parser MUST surface UID values exactly as written in the source,
without case normalisation.

## Spans

r[span.every-node]
The parser MUST attach a `Span` (start byte offset, end byte offset,
1-indexed start line, 1-indexed start column) to every AST node
(`Document`, `Section`, `Requirement`, `Field`, and each `FieldValue`
variant).

r[span.utf8-correct]
Span byte offsets MUST refer to byte positions in the original UTF-8
input. Multi-byte characters MUST NOT corrupt subsequent offsets.

## Errors

r[err.line-col]
Every `ParseError` MUST carry a 1-indexed line, a 1-indexed column, and a
0-indexed byte offset locating the error in the input.

r[err.fatal]
The v0.1 parser MUST stop at the first syntax error and return that error
to the caller. Recovery is out of scope for v0.1.

## Source annotations

r[ann.minimal]
`parse_relation_annotation` MUST accept input of the form `@relation(UID)`
and return a `RelationAnnotation` whose `scope` is `Function` (the
StrictDoc default) and whose `role` is `None`.

r[ann.scope]
`parse_relation_annotation` MUST recognise an explicit
`scope=function|file|line` keyword argument and return the corresponding
`RelationScope` variant.

r[ann.role]
`parse_relation_annotation` MUST recognise an optional
`role=Implements|Verifies|Refines` keyword argument and surface unknown
role values as `RelationRole::Other(String)`.

r[ann.multi-uid]
`parse_relation_annotation` MUST accept multiple comma-separated UID
tokens before any keyword arguments and surface them as `uids: Vec<String>`.

r[ann.span]
The returned `RelationAnnotation` MUST carry byte offsets (`start`, `end`)
delimiting the full `@relation(...)` call site within the input.

r[ann.non-match]
`parse_relation_annotation` MUST return `None` when the input does not
contain a syntactically valid `@relation(...)` call. It MUST NOT panic or
error.
