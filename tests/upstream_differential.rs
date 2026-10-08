// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Differential test against upstream StrictDoc.
//!
//! `tests/fixtures/upstream-oracle/{sdoc,markers}.jsonl` record what
//! upstream StrictDoc (the release matching the corpus submodule, see
//! `VERSION`) makes of every `.sdoc` file and every `@relation` marker in
//! its own test corpus. Regenerate them with
//! `tools/upstream-oracle/regenerate.sh` after bumping the submodule.
//!
//! These tests parse the same inputs with this crate and fail on any
//! difference that isn't listed, with a reason, in `known-divergences.txt`.
//! They also fail on stale entries in that list, so it stays accurate.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use strictdoc_parser::{find_relation_annotations, parse, Document, DocumentChild, FieldValue};

fn oracle_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/upstream-oracle")
}

fn read_jsonl(name: &str) -> Vec<Value> {
    let text = std::fs::read_to_string(oracle_dir().join(name)).expect("oracle file");
    text.lines()
        .map(|l| serde_json::from_str(l).expect("valid JSON line"))
        .collect()
}

/// `(category, key)` → reason, from `known-divergences.txt`. Lines look like
/// `category | key | reason`; `\n` in a key stands for a line break.
fn known_divergences() -> BTreeMap<(String, String), String> {
    let text =
        std::fs::read_to_string(oracle_dir().join("known-divergences.txt")).unwrap_or_default();
    let mut out = BTreeMap::new();
    for line in text.lines() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.splitn(3, " | ").collect();
        assert_eq!(parts.len(), 3, "malformed known-divergences line: {line}");
        out.insert(
            (parts[0].to_string(), parts[1].replace("\\n", "\n")),
            parts[2].to_string(),
        );
    }
    out
}

#[derive(Default)]
struct Report {
    /// `(category, key)` → detail.
    divergences: BTreeMap<(String, String), String>,
}

impl Report {
    fn add(&mut self, category: &str, key: &str, detail: String) {
        self.divergences
            .entry((category.to_string(), key.to_string()))
            .or_insert(detail);
    }

    /// Panic on divergences missing from the known list, and on known
    /// entries for this test's categories that no longer diverge.
    fn check(self, categories: &[&str]) {
        let known = known_divergences();
        let unexpected: Vec<_> = self
            .divergences
            .iter()
            .filter(|(k, _)| !known.contains_key(k))
            .collect();
        let stale: Vec<_> = known
            .keys()
            .filter(|k| categories.contains(&k.0.as_str()) && !self.divergences.contains_key(*k))
            .collect();
        eprintln!(
            "{} divergence(s) from upstream, {} known, {} unexpected, {} stale",
            self.divergences.len(),
            self.divergences.len() - unexpected.len(),
            unexpected.len(),
            stale.len()
        );
        let mut msg = String::new();
        for ((category, key), detail) in &unexpected {
            msg += &format!("\n  {category} | {} | {detail}", key.replace('\n', "\\n"));
        }
        for (category, key) in &stale {
            msg += &format!(
                "\n  stale known divergence: {category} | {}",
                key.replace('\n', "\\n")
            );
        }
        assert!(msg.is_empty(), "differences from upstream StrictDoc:{msg}");
    }
}

/// Flatten a parsed document into the shape `dump.py` produces.
fn crate_nodes(doc: &Document) -> Vec<Value> {
    fn walk(children: &[DocumentChild], path: &[String], parent: Value, out: &mut Vec<Value>) {
        for child in children {
            match child {
                DocumentChild::Section(s) => {
                    let mut path = path.to_vec();
                    path.push(s.title.clone());
                    walk(&s.children, &path, parent.clone(), out);
                }
                DocumentChild::DocumentFromFile(d) => out.push(json!({
                    "type": "DOCUMENT_FROM_FILE", "path": path, "parent": parent, "file": d.file,
                })),
                DocumentChild::Node(n) => {
                    let fields: Vec<Value> = n
                        .fields
                        .iter()
                        .map(|f| {
                            let multiline = matches!(f.value, FieldValue::Heredoc { .. });
                            json!([f.name, f.value.text(), multiline])
                        })
                        .collect();
                    let relations: Vec<Value> = n
                        .relations
                        .iter()
                        .map(|r| json!({"type": r.relation_type, "role": r.role, "target": r.target()}))
                        .collect();
                    out.push(json!({
                        "type": n.node_type, "path": path, "parent": parent,
                        "composite": n.composite, "fields": fields, "relations": relations,
                    }));
                    let index = json!(out.len() - 1);
                    walk(&n.children, path, index, out);
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(&doc.body, &[], Value::Null, &mut out);
    out
}

/// Apply the crate's documented normalisations to upstream's values:
/// single-line values are trimmed, heredoc text has no final newline.
fn normalise_upstream(mut node: Value) -> Value {
    if let Some(fields) = node.get_mut("fields").and_then(Value::as_array_mut) {
        for field in fields {
            let multiline = field[2].as_bool().unwrap_or(false);
            let text = field[1].as_str().unwrap_or_default();
            let text = if multiline {
                text.strip_suffix('\n').unwrap_or(text)
            } else {
                text.trim()
            };
            field[1] = json!(text);
        }
    }
    node
}

#[test]
fn sdoc_corpus_matches_upstream() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/upstream/strictdoc");
    if !corpus.is_dir() || !oracle_dir().is_dir() {
        eprintln!(
            "skipping: {} not found (submodule not initialised?)",
            corpus.display()
        );
        return;
    }

    let mut report = Report::default();
    let (mut parsed, mut total) = (0, 0);
    for record in read_jsonl("sdoc.jsonl") {
        let file = record["file"].as_str().unwrap();
        let Ok(content) = std::fs::read_to_string(corpus.join(file)) else {
            continue;
        };
        total += 1;
        let upstream_ok = record["ok"].as_bool().unwrap();
        let doc = match (parse(&content), upstream_ok) {
            (Ok(doc), true) => doc,
            (Ok(_), false) => {
                parsed += 1;
                let error = record["error"].as_str().unwrap_or_default();
                report.add("crate-accepts", file, format!("upstream: {error}"));
                continue;
            }
            (Err(e), true) => {
                report.add("crate-rejects", file, e.to_string());
                continue;
            }
            (Err(_), false) => continue,
        };
        parsed += 1;

        for (key, up) in record["doc"].as_object().unwrap() {
            let up = up.as_str().map(str::trim_end);
            let ours = match key.as_str() {
                "title" => doc.title.as_deref(),
                "uid" => doc.uid.as_deref(),
                "version" => doc.version.as_deref(),
                "date" => doc.date.as_deref(),
                "classification" => doc.classification.as_deref(),
                "prefix" => doc.prefix.as_deref(),
                _ => continue,
            };
            if up != ours {
                report.add(
                    "header",
                    file,
                    format!("{key}: upstream {up:?}, crate {ours:?}"),
                );
            }
        }

        let ours = crate_nodes(&doc);
        let theirs: Vec<Value> = record["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .cloned()
            .map(normalise_upstream)
            .collect();
        if ours.len() != theirs.len() {
            report.add(
                "nodes",
                file,
                format!("upstream has {} nodes, crate {}", theirs.len(), ours.len()),
            );
        } else if let Some((i, (a, b))) = theirs
            .iter()
            .zip(&ours)
            .enumerate()
            .find(|(_, (a, b))| a != b)
        {
            report.add("nodes", file, format!("node #{i}: upstream {a}, crate {b}"));
        }
    }
    eprintln!("upstream corpus: crate parses {parsed}/{total} .sdoc files");
    report.check(&["crate-accepts", "crate-rejects", "header", "nodes"]);
}

#[test]
fn relation_markers_match_upstream() {
    if !oracle_dir().is_dir() {
        // Not shipped in the crates.io package.
        eprintln!("skipping: {} not found", oracle_dir().display());
        return;
    }
    let mut report = Report::default();
    for record in read_jsonl("markers.jsonl") {
        let input = record["input"].as_str().unwrap();
        let results = find_relation_annotations(input);
        if record.get("error").is_some() {
            if !results.iter().any(Result::is_err) {
                report.add("marker", input, "upstream rejects, crate accepts".into());
            }
            continue;
        }
        let ours: Result<Vec<Value>, String> = results
            .into_iter()
            .map(|r| {
                r.map(|a| json!({"uids": a.uids, "scope": a.scope.map(|s| s.as_str()), "role": a.role}))
                    .map_err(|e| e.to_string())
            })
            .collect();
        match ours {
            Err(e) => report.add("marker", input, format!("crate rejects: {e}")),
            Ok(ours) if Value::Array(ours.clone()) != record["markers"] => report.add(
                "marker",
                input,
                format!(
                    "upstream {}, crate {}",
                    record["markers"],
                    Value::Array(ours)
                ),
            ),
            Ok(_) => {}
        }
    }
    report.check(&["marker"]);
}
