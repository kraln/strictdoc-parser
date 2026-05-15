# strictdoc-parser

A Rust parser for [StrictDoc](https://strictdoc.readthedocs.io/) `.sdoc`
files and StrictDoc-style `@relation(...)` source-comment annotations. No
runtime dependencies; designed for embedding in coverage tools, LSPs, and
CI checks that need to read StrictDoc without shelling out to the upstream
Python implementation.

Tested against the upstream
[`strictdoc-project/strictdoc`](https://github.com/strictdoc-project/strictdoc)
test corpus: **1434 / 1436** files parse cleanly at tag `0.21.0`.

## Install

```toml
[dependencies]
strictdoc-parser = "0.1"
```

## Usage

```rust
use strictdoc_parser::{parse, parse_relation_annotation, RelationScope};

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

let ann = parse_relation_annotation("// @relation(EX-001, scope=function, role=Implements)")
    .unwrap();
assert_eq!(ann.uids, vec!["EX-001"]);
assert_eq!(ann.scope, RelationScope::Function);
# Ok::<(), strictdoc_parser::ParseError>(())
```

Every AST node carries a `Span` with byte-offset and 1-indexed line/column,
so the output is suitable for hover-style IDE integrations.

## Scope of v0.1

**Supported:** `[DOCUMENT]` headers, `[GRAMMAR]` (`IMPORT_FROM_FILE:` is
surfaced; inline grammars are treated as opaque), `[[SECTION]]` blocks with
arbitrary nesting, `[REQUIREMENT]` blocks with single-line and heredoc
(`>>>` … `<<<`) field values, `@relation(UID[, …][, scope=…][, role=…])`
source annotations. Unknown blocks (e.g. `[TEXT]`,
`[[COMPOSITE_REQUIREMENT]]`, user-grammar tags) are skipped tolerantly so
the parser doesn't choke on documents that mix conventional and
custom-grammar content.

**Not supported (yet):** validation against an external `.sgra` grammar
file, round-trip serialisation, ReqIF / HTML / PDF export. For those, use
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

[MPL-2.0](LICENSE).
