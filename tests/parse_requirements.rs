// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use strictdoc_parser::{parse, FieldValue, ParseErrorKind};

const MULTILINE: &str = include_str!("fixtures/multiline-fields.sdoc");

// r[verify req.fields]
#[test]
fn parses_single_line_and_heredoc_fields_uniformly() {
    let doc = parse(MULTILINE).expect("must parse");
    let req = doc.requirements_flat().into_iter().next().unwrap();
    let comment = req.requirement.field("COMMENT").unwrap();
    assert!(matches!(comment.value, FieldValue::SingleLine { .. }));
    let statement = req.requirement.field("STATEMENT").unwrap();
    assert!(matches!(statement.value, FieldValue::Heredoc { .. }));
}

// r[verify req.heredoc-verbatim]
#[test]
fn heredoc_preserves_interior_blank_lines_and_whitespace() {
    let doc = parse(MULTILINE).expect("must parse");
    let req = doc.requirements_flat().into_iter().next().unwrap();
    let statement = req.statement().expect("STATEMENT present");
    let expected = "First paragraph of the statement.\n\nSecond paragraph, after a blank line.";
    assert_eq!(statement, expected);

    let rationale = req.requirement.field_text("RATIONALE").unwrap();
    // The interior multi-space sequence must be preserved verbatim.
    assert!(rationale.contains("preserves    interior   whitespace."));
}

// r[verify err.fatal]
// r[verify err.line-col]
#[test]
fn unterminated_heredoc_errors_with_position() {
    let input =
        "[DOCUMENT]\nTITLE: T\n\n[REQUIREMENT]\nUID: X\nSTATEMENT: >>>\nopen but never closed\n";
    let err = parse(input).unwrap_err();
    assert_eq!(err.kind, ParseErrorKind::UnterminatedHeredoc);
    assert_eq!(err.line, 6); // STATEMENT: >>> line
    assert!(err.column >= 1);
}

// r[verify span.every-node]
#[test]
fn every_node_carries_a_span() {
    let doc = parse(MULTILINE).expect("must parse");
    assert_eq!(doc.span.start, 0);
    assert_eq!(doc.span.end, MULTILINE.len());
    let req = match &doc.body[0] {
        strictdoc_parser::DocumentChild::Node(r) => r,
        _ => panic!("expected req"),
    };
    assert!(req.span.start < req.span.end);
    for field in &req.fields {
        assert!(
            field.span.start < field.span.end,
            "field {} span empty",
            field.name
        );
        let value_span = field.value.span();
        assert!(value_span.start <= value_span.end);
    }
}
