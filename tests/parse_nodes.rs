// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use strictdoc_parser::{parse, DocumentChild, Node, ParseErrorKind};

fn node(child: &DocumentChild) -> &Node {
    match child {
        DocumentChild::Node(n) => n,
        other => panic!("expected node, got {other:?}"),
    }
}

// r[verify node.any-tag]
#[test]
fn any_tag_becomes_a_node() {
    let input = "\
[DOCUMENT]
TITLE: T

[TEXT]
STATEMENT: Intro text.

[FEATURE]
UID: FEAT-1
Verified_by: Bob

[LOW_LEVEL_REQUIREMENT2]
UID: LLR-1
";
    let doc = parse(input).expect("must parse");
    let types: Vec<&str> = doc
        .body
        .iter()
        .map(|c| node(c).node_type.as_str())
        .collect();
    assert_eq!(types, ["TEXT", "FEATURE", "LOW_LEVEL_REQUIREMENT2"]);
    assert_eq!(node(&doc.body[1]).field_text("Verified_by"), Some("Bob"));
    assert!(!node(&doc.body[1]).composite);
}

// r[verify node.composite]
#[test]
fn composite_nodes_hold_children() {
    let input = "\
[DOCUMENT]
TITLE: T

[[COMPOSITE_REQUIREMENT]]
UID: C-1
TITLE: Parent

[REQUIREMENT]
UID: C-1.1

[[COMPOSITE_REQUIREMENT]]
UID: C-1.2

[REQUIREMENT]
UID: C-1.2.1

[[/COMPOSITE_REQUIREMENT]]

[[/COMPOSITE_REQUIREMENT]]
";
    let doc = parse(input).expect("must parse");
    let parent = node(&doc.body[0]);
    assert!(parent.composite);
    assert_eq!(parent.field_text("UID"), Some("C-1"));
    assert_eq!(parent.children.len(), 2);
    let inner = node(&parent.children[1]);
    assert!(inner.composite);
    assert_eq!(node(&inner.children[0]).field_text("UID"), Some("C-1.2.1"));
    let span_text = &input[parent.span.start..parent.span.end];
    assert!(span_text.starts_with("[[COMPOSITE_REQUIREMENT]]"));
    assert!(span_text.ends_with("[[/COMPOSITE_REQUIREMENT]]"));
}

// r[verify node.flat]
#[test]
fn requirements_flat_covers_normative_nodes() {
    let input = "\
[DOCUMENT]
TITLE: T

[[SECTION]]
TITLE: S

[TEXT]
STATEMENT: Not normative.

[[COMPOSITE_REQUIREMENT]]
UID: C-1

[REQUIREMENT]
UID: C-1.1

[[/COMPOSITE_REQUIREMENT]]

[FEATURE]
UID: F-1

[[/SECTION]]
";
    let doc = parse(input).expect("must parse");
    let reqs = doc.requirements_flat();
    let uids: Vec<_> = reqs.iter().map(|r| r.uid().unwrap()).collect();
    assert_eq!(uids, ["C-1", "C-1.1", "F-1"]);
    assert!(reqs.iter().all(|r| r.section_path == ["S"]));
    assert_eq!(reqs[2].node_type(), "FEATURE");
    assert_eq!(doc.nodes_flat().len(), 4);
}

// r[verify node.relations]
#[test]
fn relations_are_parsed() {
    let input = "\
[DOCUMENT]
TITLE: T

[REQUIREMENT]
UID: R-2
STATEMENT: x
RELATIONS:
- TYPE: Parent
  VALUE: R-1
  ROLE: Refines
- TYPE: File
  VALUE: src/lib.rs
  LINE_RANGE: 1, 5
";
    let doc = parse(input).expect("must parse");
    let req = node(&doc.body[0]);
    assert!(req.field("RELATIONS").is_none());
    assert_eq!(req.relations.len(), 2);
    let parent = &req.relations[0];
    assert_eq!(parent.relation_type, "Parent");
    assert_eq!(parent.target(), Some("R-1"));
    assert_eq!(parent.role.as_deref(), Some("Refines"));
    let file = &req.relations[1];
    assert_eq!(file.target(), Some("src/lib.rs"));
    assert_eq!(file.property("LINE_RANGE"), Some("1, 5"));
    assert!(input[file.span.start..file.span.end].ends_with("LINE_RANGE: 1, 5"));
}

#[test]
fn relation_without_type_is_an_error() {
    let input = "[DOCUMENT]\nTITLE: T\n\n[REQUIREMENT]\nUID: R\nRELATIONS:\n- VALUE: X\n";
    let err = parse(input).unwrap_err();
    assert_eq!(err.kind, ParseErrorKind::MalformedRelation);
    assert_eq!(err.line, 7);
}

// r[verify req.heredoc-close]
#[test]
fn indented_heredoc_closer_is_text() {
    let input = "\
[DOCUMENT]
TITLE: T

[TEXT]
STATEMENT: >>>
.. code:: strictdoc

    [REQUIREMENT]
    STATEMENT: >>>
    Example.
    <<<
<<<
";
    let doc = parse(input).expect("must parse");
    let text = node(&doc.body[0]).field_text("STATEMENT").unwrap();
    assert!(text.ends_with("    Example.\n    <<<"), "{text:?}");
}

// r[verify req.heredoc-verbatim]
#[test]
fn heredoc_keeps_leading_and_trailing_blank_lines() {
    let input = "[DOCUMENT]\nTITLE: T\n\n[TEXT]\nSTATEMENT: >>>\n\nbody\n\n<<<\n";
    let doc = parse(input).expect("must parse");
    assert_eq!(node(&doc.body[0]).field_text("STATEMENT"), Some("\nbody\n"));
}

// r[verify doc.from-file]
#[test]
fn document_from_file_is_surfaced() {
    let input = "[DOCUMENT]\nTITLE: T\n\n[DOCUMENT_FROM_FILE]\nFILE: nested.sdoc\n";
    let doc = parse(input).expect("must parse");
    match &doc.body[0] {
        DocumentChild::DocumentFromFile(d) => assert_eq!(d.file, "nested.sdoc"),
        other => panic!("expected include, got {other:?}"),
    }
}

// r[verify doc.req-prefix]
#[test]
fn req_prefix_is_the_document_prefix() {
    let doc = parse("[DOCUMENT]\nTITLE: T\nREQ_PREFIX: SYS-\n").expect("must parse");
    assert_eq!(doc.prefix.as_deref(), Some("SYS-"));
}

// r[verify doc.metadata]
#[test]
fn metadata_block_is_collected() {
    let input = "[DOCUMENT]\nTITLE: T\nMETADATA:\n  author: Jane\n  Reviewed-By: Joe\n";
    let doc = parse(input).expect("must parse");
    assert_eq!(doc.metadata.get("author").map(String::as_str), Some("Jane"));
    assert_eq!(
        doc.metadata.get("Reviewed-By").map(String::as_str),
        Some("Joe")
    );
}

// r[verify sect.legacy]
#[test]
fn legacy_section_form_is_accepted() {
    let input = "\
[DOCUMENT]
TITLE: T

[SECTION]
TITLE: Old style

[REQUIREMENT]
UID: R-1

[/SECTION]
";
    let doc = parse(input).expect("must parse");
    let reqs = doc.requirements_flat();
    assert_eq!(reqs[0].section_path, ["Old style"]);
}
