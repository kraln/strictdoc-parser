// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use strictdoc_parser::{parse, DocumentChild, ParseErrorKind};

// r[verify sect.open-close]
#[test]
fn unclosed_section_errors_out() {
    let input = "[DOCUMENT]\nTITLE: T\n\n[[SECTION]]\nTITLE: Outer\n\n[REQUIREMENT]\nUID: U-1\nSTATEMENT: x\n";
    let err = parse(input).unwrap_err();
    assert_eq!(err.kind, ParseErrorKind::UnclosedSection);
    // Error should point at the [[SECTION]] line.
    assert_eq!(err.line, 4);
}

// r[verify sect.title]
#[test]
fn section_without_title_errors_out() {
    let input = "[DOCUMENT]\nTITLE: T\n\n[[SECTION]]\n[[/SECTION]]\n";
    let err = parse(input).unwrap_err();
    assert_eq!(err.kind, ParseErrorKind::MissingSectionTitle);
}

// r[verify sect.nesting]
#[test]
fn supports_four_level_section_nesting() {
    let input = "\
[DOCUMENT]
TITLE: T

[[SECTION]]
TITLE: L1
[[SECTION]]
TITLE: L2
[[SECTION]]
TITLE: L3
[[SECTION]]
TITLE: L4

[REQUIREMENT]
UID: DEEP-1
STATEMENT: deep

[[/SECTION]]
[[/SECTION]]
[[/SECTION]]
[[/SECTION]]
";
    let doc = parse(input).expect("must parse");
    // Walk to depth 4 manually.
    let s1 = match &doc.body[0] {
        DocumentChild::Section(s) => s,
        _ => panic!("expected section"),
    };
    assert_eq!(s1.title, "L1");
    let s2 = match &s1.children[0] {
        DocumentChild::Section(s) => s,
        _ => panic!("L2"),
    };
    let s3 = match &s2.children[0] {
        DocumentChild::Section(s) => s,
        _ => panic!("L3"),
    };
    let s4 = match &s3.children[0] {
        DocumentChild::Section(s) => s,
        _ => panic!("L4"),
    };
    let req = match &s4.children[0] {
        DocumentChild::Requirement(r) => r,
        _ => panic!("expected req"),
    };
    assert_eq!(req.field_text("UID"), Some("DEEP-1"));
}
