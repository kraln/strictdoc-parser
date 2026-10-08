# strictdoc-parser

A Rust parser for [StrictDoc](https://strictdoc.readthedocs.io/) `.sdoc`
files and StrictDoc-style `@relation(...)` source-comment annotations. No
runtime dependencies; designed for embedding in coverage tools, LSPs, and
CI checks that need to read StrictDoc without shelling out to the upstream
Python implementation.

Tested differentially against upstream
[`strictdoc-project/strictdoc`](https://github.com/strictdoc-project/strictdoc)
`0.21.0` on its own test corpus: for every `.sdoc` file upstream accepts,
the crate produces the same nodes, fields, relations and section structure,
and every `@relation` marker in the corpus parses to the same UIDs, scope
and role. The few intended differences (mostly upstream semantic checks the
crate doesn't do) are listed in
[`tests/fixtures/upstream-oracle/known-divergences.txt`](https://github.com/kraln/strictdoc-parser/blob/main/tests/fixtures/upstream-oracle/known-divergences.txt).

## Install

```toml
[dependencies]
strictdoc-parser = "0.2"
```

## Usage

```rust
use strictdoc_parser::{parse, parse_relation_annotation, RelationRole, RelationScope};

let src = "\
[DOCUMENT]
TITLE: Example
UID: EX-DOC

[REQUIREMENT]
UID: EX-001
TITLE: Hello
STATEMENT: >>>
The system shall greet the world.
<<<
";

let doc = parse(src)?;
for req in doc.requirements_flat() {
    println!("{}: {}", req.uid().unwrap_or(""), req.title().unwrap_or(""));
}

let ann = parse_relation_annotation("// @relation(EX-001, scope=function, role=Implementation)")
    .unwrap();
assert_eq!(ann.uids, vec!["EX-001"]);
assert_eq!(ann.scope, Some(RelationScope::Function));
assert_eq!(ann.role.as_deref(), Some("Implementation"));
assert_eq!(ann.role_kind(), Some(RelationRole::Implements));
# Ok::<(), strictdoc_parser::ParseError>(())
```

Every AST node carries a `Span` with byte-offset and 1-indexed line/column,
so the output is suitable for hover-style IDE integrations.

## Scope

**Supported:**

- `[DOCUMENT]` headers, including `OPTIONS:` and `METADATA:`.
- `[GRAMMAR]`: `IMPORT_FROM_FILE:` is surfaced; inline grammars are
  treated as opaque.
- `[[SECTION]]` blocks with arbitrary nesting, and the legacy `[SECTION]`
  form.
- Nodes of any element type (`[REQUIREMENT]`, `[TEXT]`, custom-grammar
  elements), composite `[[TAG]]` … `[[/TAG]]` nodes, single-line and heredoc
  (`>>>` … `<<<`) fields, and `RELATIONS:` lists.
- `[DOCUMENT_FROM_FILE]` includes (surfaced, not followed).
- `@relation(…)` / `@relation{…}` source markers with every upstream scope
  (`file`, `class`, `function`, `line`, `range_start`, `range_end`) and
  free-form roles. `find_relation_annotations` reports malformed markers
  as errors.

**Not supported (yet):** validation against a grammar (inline or `.sgra`),
following includes, round-trip serialisation, and ReqIF / HTML / PDF export.
For those, use
[upstream StrictDoc](https://github.com/strictdoc-project/strictdoc).

## Features

- `default`: zero runtime dependencies.
- `serde`: derive `Serialize`/`Deserialize` on AST and annotation types.

## Authoring note

Code, tests, requirements, and documentation in this repository were
produced with substantial assistance from an LLM (Anthropic's Claude)
under human direction. All output was reviewed before commit; the
upstream-corpus pass rate above reflects actual runs against the real
[`strictdoc-project/strictdoc`](https://github.com/strictdoc-project/strictdoc)
test suite, not a generated estimate.

## License

[MPL-2.0](https://github.com/kraln/strictdoc-parser/blob/main/LICENSE).
