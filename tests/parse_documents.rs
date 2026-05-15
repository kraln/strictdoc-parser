// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use strictdoc_parser::parse;

const MINIMAL: &str = include_str!("fixtures/minimal.sdoc");
const NESTED: &str = include_str!("fixtures/nested-sections.sdoc");
const MULTILINE: &str = include_str!("fixtures/multiline-fields.sdoc");

// r[verify doc.header]
// r[verify doc.fields]
#[test]
fn parses_document_header() {
    let doc = parse(MINIMAL).expect("must parse");
    assert_eq!(doc.title.as_deref(), Some("Minimal Example"));
    assert_eq!(doc.uid.as_deref(), Some("TEST-MIN"));
    assert_eq!(doc.version.as_deref(), Some("0.1"));
    assert_eq!(doc.date.as_deref(), Some("2024-01-01"));
    assert_eq!(doc.classification.as_deref(), Some("Public"));
    assert_eq!(doc.prefix.as_deref(), Some("TEST"));
}

// r[verify doc.options]
#[test]
fn parses_options_subblock() {
    let doc = parse(MINIMAL).expect("must parse");
    assert_eq!(doc.options.get("MARKUP").map(String::as_str), Some("Text"));
    assert_eq!(
        doc.options.get("AUTO_LEVELS").map(String::as_str),
        Some("On")
    );
    assert_eq!(
        doc.options.get("NODE_IN_TOC").map(String::as_str),
        Some("True")
    );
}

// r[verify doc.grammar-import]
#[test]
fn parses_grammar_import_path() {
    let doc = parse(MINIMAL).expect("must parse");
    assert_eq!(doc.grammar_import.as_deref(), Some("grammar.sgra"));
}

// r[verify req.uid-verbatim]
#[test]
fn preserves_uid_case_verbatim() {
    let input = "[DOCUMENT]\nTITLE: T\n\n[REQUIREMENT]\nUID: Mixed-Case-001\nSTATEMENT: x\n";
    let doc = parse(input).expect("must parse");
    let reqs = doc.requirements_flat();
    assert_eq!(reqs[0].uid(), Some("Mixed-Case-001"));
}

// r[verify span.utf8-correct]
#[test]
fn spans_track_utf8_byte_offsets() {
    let input =
        "[DOCUMENT]\nTITLE: μ-broker spec\nUID: T\n\n[REQUIREMENT]\nUID: U-1\nSTATEMENT: y\n";
    let doc = parse(input).expect("must parse");
    // Title field is on line 2, after the 'μ' multi-byte char.
    assert_eq!(doc.title.as_deref(), Some("μ-broker spec"));
    let reqs = doc.requirements_flat();
    assert_eq!(reqs.len(), 1);
    // Requirement span should start at the [REQUIREMENT] line, not corrupted by μ.
    let req_span = reqs[0].requirement.span;
    let req_text = &input[req_span.start..];
    assert!(req_text.starts_with("[REQUIREMENT]"));
}

// r[verify req.field-order]
#[test]
fn preserves_field_order() {
    let doc = parse(MULTILINE).expect("must parse");
    let req = match &doc.body[0] {
        strictdoc_parser::DocumentChild::Requirement(r) => r,
        _ => panic!("expected requirement"),
    };
    let names: Vec<&str> = req.fields.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, vec!["UID", "STATEMENT", "RATIONALE", "COMMENT"]);
}

#[test]
fn nested_section_fixture_smoke() {
    let doc = parse(NESTED).expect("must parse");
    let reqs = doc.requirements_flat();
    assert_eq!(reqs.len(), 3);
    let uids: Vec<&str> = reqs.iter().map(|r| r.uid().unwrap()).collect();
    assert_eq!(uids, vec!["TEST-OUT-1", "TEST-IN-1", "TEST-DEEP-1"]);
}
