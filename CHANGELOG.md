# Changelog

## [0.2.0] - Unreleased

- **Breaking:** `.sdoc` elements of any tag (`[TEXT]`, custom-grammar elements such as `[FEATURE]`) and composite `[[TAG]]` … `[[/TAG]]` blocks are now parsed as `Node`s instead of being skipped; `[REQUIREMENT]`s nested in composites are no longer lost. `Requirement` is renamed `Node` (deprecated alias kept), and `DocumentChild::Requirement` is now `DocumentChild::Node`.
- `requirements_flat()` now returns every normative node (all except `[TEXT]`); new `nodes_flat()` returns all nodes.
- `RELATIONS:` lists are parsed into `Node::relations` (`Relation` with type, role, properties).
- `[DOCUMENT_FROM_FILE]` is surfaced as `DocumentChild::DocumentFromFile`.
- Legacy `[SECTION]` … `[/SECTION]` blocks are accepted as sections.
- Sections keep all their header fields (`Section::fields`).
- `REQ_PREFIX:` is read as the document prefix; `METADATA:` is collected into `Document::metadata`; a leading UTF-8 BOM is skipped.
- Fix: an indented `<<<` (e.g. in a code example) no longer ends a heredoc; block markers and `<<<` are only recognised at column 1, as upstream.
- Heredoc text is now verbatim (leading/trailing blank lines are kept).
- Field names follow upstream's `[A-Z][A-Za-z0-9_-]*`.
- **Breaking:** `ParseErrorKind` is `#[non_exhaustive]` with reworked variants (`UnmatchedClose`, `MismatchedClose`, `UnclosedBlock`, `MalformedRelation`).
- **Breaking:** `@relation` markers: `scope` is now `Option<RelationScope>` (no implied `function`), `RelationScope` gains `Class`, `RangeStart`, `RangeEnd`, and `role` is the verbatim `Option<String>`; `role_kind()` / `RelationRole::classify` map verb and noun forms (`Implementation`, `Test`, …).
- `@relation{…}` (Doxygen-friendly) form is accepted.
- Marker syntax follows upstream's grammar (UID syntax, argument order, `, ` separators); markers must start a comment line, so mentions in prose are ignored.
- New `find_relation_annotations()` returns every marker, with `AnnotationError`s for malformed ones instead of silently dropping them.
- The README example is now compiled and run as a doctest.
- Tests: a differential test compares the crate against upstream StrictDoc 0.21.0 on its whole test corpus (`.sdoc` structure and `@relation` markers), replacing the parse-only corpus smoke test. Expected output is regenerated with `tools/upstream-oracle/regenerate.sh`; intended differences are listed in `tests/fixtures/upstream-oracle/known-divergences.txt`.

## [0.1.1] - 2026-05-15

Initial release. (0.1.0 was tagged but never published.)
