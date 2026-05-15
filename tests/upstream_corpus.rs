// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Smoke test over the upstream strictdoc-project/strictdoc corpus.
//!
//! Walks `tests/fixtures/upstream/strictdoc/` for every `.sdoc` file and
//! exercises [`strictdoc_parser::parse`] on each. Two modes:
//!
//! - **Default**: emits a coverage report (parsed / failed counts) but does
//!   not assert all files succeed. v0.1 of the parser intentionally
//!   targets the most-common subset of the StrictDoc grammar; some
//!   upstream fixtures use experimental constructs (`[FREETEXT]`, custom
//!   grammars, …) we don't yet support. The report flags regressions.
//! - **Strict** (`STRICTDOC_PARSER_STRICT_CORPUS=1`): asserts every file
//!   parses without error. Useful once the parser claims full coverage.
//!
//! Either way the test must not panic; parser errors are caught and
//! tallied.

use std::path::{Path, PathBuf};

use strictdoc_parser::parse;

fn corpus_root() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join("tests/fixtures/upstream/strictdoc")
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else if path.extension().and_then(|s| s.to_str()) == Some("sdoc") {
            out.push(path);
        }
    }
}

#[test]
fn upstream_corpus_smoke() {
    let root = corpus_root();
    if !root.is_dir() {
        eprintln!(
            "skipping upstream corpus test: {} not found (submodule not initialised?)",
            root.display()
        );
        return;
    }

    let mut files = Vec::new();
    walk(&root, &mut files);
    files.sort();
    assert!(!files.is_empty(), "no .sdoc files under {}", root.display());

    let strict = std::env::var("STRICTDOC_PARSER_STRICT_CORPUS").is_ok();

    let mut ok = 0usize;
    let mut failed: Vec<(PathBuf, strictdoc_parser::ParseError)> = Vec::new();

    for path in &files {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("warning: failed to read {}: {}", path.display(), e);
                continue;
            }
        };
        match parse(&content) {
            Ok(_) => ok += 1,
            Err(e) => failed.push((path.clone(), e)),
        }
    }

    let total = ok + failed.len();
    eprintln!(
        "upstream corpus: {ok}/{total} files parsed ({} failed)",
        failed.len()
    );

    if !failed.is_empty() {
        // Print up to 10 failures for triage.
        for (path, err) in failed.iter().take(10) {
            let rel = path.strip_prefix(&root).unwrap_or(path);
            eprintln!("  FAIL {}: {}", rel.display(), err);
        }
        if failed.len() > 10 {
            eprintln!("  …and {} more", failed.len() - 10);
        }
    }

    if strict {
        assert!(
            failed.is_empty(),
            "{} corpus files failed to parse under STRICTDOC_PARSER_STRICT_CORPUS=1",
            failed.len()
        );
    } else {
        // Regression guard: even in non-strict mode, demand the parser
        // handles the bulk of the upstream corpus. Adjust this floor only
        // when a submodule bump introduces genuinely new constructs the
        // parser can't yet handle.
        let pass_rate = ok as f64 / total as f64;
        assert!(
            pass_rate >= 0.99,
            "upstream corpus pass rate dropped below 99%: {ok}/{total} ({:.2}%)",
            pass_rate * 100.0
        );
    }
}
