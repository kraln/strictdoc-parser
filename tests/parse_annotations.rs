// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use strictdoc_parser::{parse_relation_annotation, RelationRole, RelationScope};

// Most behaviour is exercised by the unit tests in src/annotation.rs;
// this integration suite covers patterns a tool like tracey would feed in
// while walking real source files.

#[test]
fn ignores_unrelated_comment_text() {
    assert!(parse_relation_annotation("// TODO: revisit this later").is_none());
    assert!(parse_relation_annotation("// see @relation in docs/spec.md").is_none());
}

#[test]
fn parses_annotation_embedded_in_block_comment_line() {
    let line = " * @relation(REQ-99)  some trailing prose";
    let ann = parse_relation_annotation(line).expect("recognised");
    assert_eq!(ann.uids, vec!["REQ-99"]);
}

#[test]
fn parses_annotation_with_uppercase_uids() {
    // StrictDoc-style UIDs are uppercase; we preserve them verbatim.
    let ann = parse_relation_annotation("// @relation(BR-001, BR-002, scope=function)").unwrap();
    assert_eq!(ann.uids, vec!["BR-001", "BR-002"]);
    assert_eq!(ann.scope, RelationScope::Function);
}

#[test]
fn parses_annotation_with_role_attached() {
    let ann =
        parse_relation_annotation("/// @relation(REQ-100, scope=file, role=Verifies)").unwrap();
    assert_eq!(ann.scope, RelationScope::File);
    assert_eq!(ann.role, Some(RelationRole::Verifies));
}
