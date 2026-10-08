# strictdoc-parser — Requirements

This document is the tracey-readable specification for the
`strictdoc-parser` crate. Each `r[…]` marker defines a requirement covered
by the implementation and tests in this repository.

The reference for behaviour is upstream StrictDoc at the version pinned by
the `tests/fixtures/upstream/strictdoc` submodule. Intended differences are
listed in `tests/fixtures/upstream-oracle/known-divergences.txt`.

## Document structure

r[doc.header]
The parser MUST accept a single `[DOCUMENT]` header as the first non-blank
block in a `.sdoc` file and MUST reject input whose first block is anything
else.

r[doc.fields]
The parser MUST extract `TITLE`, `UID`, `VERSION`, `DATE`,
`CLASSIFICATION`, and `PREFIX` fields from the `[DOCUMENT]` header when
present, treating each as optional at the parser layer.

r[doc.req-prefix]
The parser MUST accept `REQ_PREFIX:` as an alternative spelling of the
`[DOCUMENT]` header's `PREFIX:` field.

r[doc.options]
The parser MUST collect the indented sub-block following an `OPTIONS:`
header into a key-value map on the parsed `Document`.

r[doc.metadata]
The parser MUST collect the indented `key: value` lines following a
`METADATA:` header into a key-value map on the parsed `Document`, accepting
lowercase keys.

r[doc.grammar-import]
The parser MUST recognise a `[GRAMMAR]` block and surface the path from
its `IMPORT_FROM_FILE:` field on the parsed `Document`. The parser MUST
NOT attempt to load or validate the referenced grammar file.

r[doc.from-file]
The parser MUST surface each `[DOCUMENT_FROM_FILE]` block as a
`DocumentChild::DocumentFromFile` carrying its `FILE:` value. The parser
MUST NOT attempt to load the referenced file.

## Sections

r[sect.open-close]
The parser MUST pair `[[SECTION]]` opens with `[[/SECTION]]` closes (and,
generally, every `[[TAG]]` with `[[/TAG]]`), MUST report a parse error
when a closing tag doesn't match the innermost open block, and MUST report
a parse error with line and column information when a block is left
unclosed at EOF.

r[sect.title]
The parser MUST require sections to carry a `TITLE:` field and MUST report
a parse error when a section has none. All section header fields MUST be
kept in source order.

r[sect.nesting]
The parser MUST support `[[SECTION]]` blocks nested inside other
`[[SECTION]]` blocks to a depth of at least four levels.

r[sect.legacy]
The parser MUST accept the legacy `[SECTION]` … `[/SECTION]` form as a
section, although upstream StrictDoc no longer does.

## Nodes

r[node.any-tag]
The parser MUST parse a `[TAG]` block with any tag matching
`[A-Z][A-Z0-9_]*` (`[REQUIREMENT]`, `[TEXT]`, custom-grammar elements such
as `[FEATURE]`) into a `Node` whose `node_type` is the tag.

r[node.composite]
The parser MUST parse a `[[TAG]]` … `[[/TAG]]` block other than
`[[SECTION]]` into a composite `Node` whose children (sections, nodes,
includes) are parsed like document-level content.

r[node.relations]
The parser MUST parse the `- TYPE: …` list following a node's
`RELATIONS:` line into `Relation` values carrying the type, the optional
`ROLE`, and every other property (`VALUE`, `PATH`, …) in source order, and
MUST NOT include `RELATIONS` among the node's fields.

r[node.flat]
`Document::requirements_flat` MUST return, in source order, every node
except `[TEXT]` nodes, including composite nodes and the nodes nested in
them, each with the titles of its enclosing sections.

r[req.fields]
The parser MUST parse single-line `KEY: value` fields and multi-line
`KEY: >>>` … `<<<` heredoc fields inside node blocks, producing a
`FieldValue::SingleLine` or `FieldValue::Heredoc` respectively. Field
names follow upstream's `[A-Z][A-Za-z0-9_-]*`.

r[req.field-order]
The parser MUST preserve the source order of fields within each node.

r[req.heredoc-verbatim]
The parser MUST preserve heredoc body content verbatim: every line
between the `KEY: >>>` line and the closing `<<<` line, joined with `\n`,
including leading and trailing blank lines.

r[req.heredoc-close]
The parser MUST only treat `<<<` as a heredoc closer when it starts at
column 1, so that indented `<<<` lines (e.g. in a code example) remain part
of the heredoc body.

r[req.uid-verbatim]
The parser MUST surface UID values exactly as written in the source,
without case normalisation.

## Spans

r[span.every-node]
The parser MUST attach a `Span` (start byte offset, end byte offset,
1-indexed start line, 1-indexed start column) to every AST node
(`Document`, `Section`, `Node`, `Relation`, `DocumentFromFile`, `Field`,
and each `FieldValue` variant).

r[span.utf8-correct]
Span byte offsets MUST refer to byte positions in the original UTF-8
input. Multi-byte characters MUST NOT corrupt subsequent offsets.

## Errors

r[err.line-col]
Every `ParseError` MUST carry a 1-indexed line, a 1-indexed column, and a
0-indexed byte offset locating the error in the input.

r[err.fatal]
The parser MUST stop at the first syntax error and return that error to
the caller. Recovery is out of scope.

## Source annotations

r[ann.minimal]
`parse_relation_annotation` MUST accept input of the form `@relation(UID)`
and return a `RelationAnnotation` whose `scope` and `role` are `None`.

r[ann.scope]
The annotation parser MUST recognise an explicit `scope=` argument with any
of the values `file`, `class`, `function`, `line`, `range_start` or
`range_end`, and return the corresponding `RelationScope` variant.

r[ann.role]
The annotation parser MUST recognise an optional `role=` argument and
surface its value verbatim.

r[ann.role-kind]
`RelationRole::classify` MUST map role values, ignoring ASCII case, to
`Implements` (`Implements`, `Implement`, `Implementation`, `Impl`),
`Verifies` (`Verifies`, `Verify`, `Verification`, `Test`, `Tests`) or
`Refines` (`Refines`, `Refine`, `Refinement`), and anything else to
`RelationRole::Other`.

r[ann.multi-uid]
The annotation parser MUST accept multiple comma-separated UID tokens
before any keyword arguments and surface them as `uids: Vec<String>`.

r[ann.uid-syntax]
The annotation parser MUST only accept UIDs matching upstream's
`[A-Za-z][A-Za-z0-9_/.-]+`.

r[ann.brace-form]
The annotation parser MUST accept `@relation{…}` as well as
`@relation(…)`.

r[ann.span]
The returned `RelationAnnotation` MUST carry byte offsets (`start`, `end`)
delimiting the full `@relation(...)` call site within the input.

r[ann.comment-start]
The annotation parser MUST only recognise a marker that starts a comment
line: preceded on its line by nothing but whitespace, a comment leader
(`//`, `#`, `*`, `--`, …), or another marker.

r[ann.non-match]
`parse_relation_annotation` MUST return `None`, and
`find_relation_annotations` an empty list, when the input contains no
`@relation(` or `@relation{` call that starts a comment line. Neither may
panic.

r[ann.errors]
`find_relation_annotations` MUST report a recognised marker that doesn't
follow upstream's marker grammar (missing or invalid UID, duplicate UID,
unknown scope, invalid role, arguments out of order, wrong separators,
missing closing brace) as an `AnnotationError`, and MUST continue scanning
after it.
