// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Parser for StrictDoc-style `@relation(...)` source-comment annotations.
//!
//! These annotations live in source-code comments and link implementation
//! sites to requirements. Examples:
//!
//! ```text
//! // @relation(BR-001)
//! // @relation(BR-001, scope=function)
//! // @relation(BR-001, scope=function, role=Implements)
//! // @relation(BR-001, BR-003, scope=function)
//! ```

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RelationAnnotation {
    /// One or more UIDs the annotation refers to (preserved verbatim).
    pub uids: Vec<String>,
    /// Annotation scope (defaults to [`RelationScope::Function`] when not
    /// specified).
    pub scope: RelationScope,
    /// Optional semantic role.
    pub role: Option<RelationRole>,
    /// Byte offsets within the input that correspond to the matched
    /// `@relation(...)` call, inclusive of `@relation` and the closing `)`.
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum RelationScope {
    Function,
    File,
    Line,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum RelationRole {
    Implements,
    Verifies,
    Refines,
    /// Any other role value, surfaced verbatim. Consumers decide whether
    /// to accept or reject.
    Other(String),
}

/// Try to parse a `@relation(UID[, UID...][, scope=...][, role=...])` call
/// out of arbitrary text (typically a comment line). Returns `None` if no
/// recognisable call is found.
///
/// Whitespace inside the parentheses and around `=` is tolerated. UID
/// tokens are taken verbatim — no case normalisation.
// r[impl ann.minimal]
// r[impl ann.scope]
// r[impl ann.role]
// r[impl ann.multi-uid]
// r[impl ann.span]
// r[impl ann.non-match]
pub fn parse_relation_annotation(input: &str) -> Option<RelationAnnotation> {
    const NEEDLE: &str = "@relation";
    let mut search_start = 0;
    while let Some(rel_offset) = input[search_start..].find(NEEDLE) {
        let abs_at = search_start + rel_offset;
        let after_marker = abs_at + NEEDLE.len();
        // Look for the opening paren (allowing whitespace between `@relation`
        // and `(`).
        let mut cursor = after_marker;
        while cursor < input.len() && input.as_bytes()[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= input.len() || input.as_bytes()[cursor] != b'(' {
            search_start = after_marker;
            continue;
        }
        cursor += 1; // past '('

        // Find the matching `)`. We don't expect nested parens in this
        // grammar, so a linear scan to the next `)` suffices.
        let inner_start = cursor;
        let inner_end = match input[inner_start..].find(')') {
            Some(o) => inner_start + o,
            None => {
                search_start = after_marker;
                continue;
            }
        };
        let inner = &input[inner_start..inner_end];

        if let Some(ann) = parse_inner(inner, abs_at, inner_end + 1) {
            return Some(ann);
        }
        search_start = inner_end + 1;
    }
    None
}

/// Parse the contents of the parentheses (without the `(`/`)` themselves).
fn parse_inner(inner: &str, call_start: usize, call_end: usize) -> Option<RelationAnnotation> {
    let mut uids: Vec<String> = Vec::new();
    let mut scope: Option<RelationScope> = None;
    let mut role: Option<RelationRole> = None;

    for raw_arg in inner.split(',') {
        let arg = raw_arg.trim();
        if arg.is_empty() {
            continue;
        }
        if let Some((key, value)) = parse_kv(arg) {
            match key {
                "scope" => {
                    scope = Some(parse_scope(value)?);
                }
                "role" => {
                    role = Some(parse_role(value));
                }
                _ => {
                    // Unknown key — leave the annotation incomplete by
                    // returning None? For forward compatibility we accept
                    // and ignore unknown keys.
                }
            }
        } else {
            // Treat as a UID token.
            uids.push(arg.to_string());
        }
    }

    if uids.is_empty() {
        return None;
    }

    Some(RelationAnnotation {
        uids,
        scope: scope.unwrap_or(RelationScope::Function),
        role,
        start: call_start,
        end: call_end,
    })
}

fn parse_kv(arg: &str) -> Option<(&str, &str)> {
    let eq_idx = arg.find('=')?;
    let key = arg[..eq_idx].trim();
    let value = arg[eq_idx + 1..].trim();
    if key.is_empty() || value.is_empty() {
        return None;
    }
    // A valid key is a lowercase identifier.
    if !key.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
        return None;
    }
    Some((key, value))
}

fn parse_scope(s: &str) -> Option<RelationScope> {
    match s {
        "function" => Some(RelationScope::Function),
        "file" => Some(RelationScope::File),
        "line" => Some(RelationScope::Line),
        _ => None,
    }
}

fn parse_role(s: &str) -> RelationRole {
    match s {
        "Implements" => RelationRole::Implements,
        "Verifies" => RelationRole::Verifies,
        "Refines" => RelationRole::Refines,
        other => RelationRole::Other(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify ann.minimal]
    #[test]
    fn minimal_form_defaults_to_function_scope() {
        let ann = parse_relation_annotation("@relation(BR-001)").expect("recognised");
        assert_eq!(ann.uids, vec!["BR-001"]);
        assert_eq!(ann.scope, RelationScope::Function);
        assert_eq!(ann.role, None);
    }

    // r[verify ann.scope]
    #[test]
    fn explicit_scope_parsed() {
        let ann = parse_relation_annotation("// @relation(BR-001, scope=file)").unwrap();
        assert_eq!(ann.scope, RelationScope::File);
        let ann = parse_relation_annotation("// @relation(BR-001, scope=line)").unwrap();
        assert_eq!(ann.scope, RelationScope::Line);
    }

    // r[verify ann.role]
    #[test]
    fn role_parsed() {
        let ann =
            parse_relation_annotation("// @relation(BR-001, scope=function, role=Implements)")
                .unwrap();
        assert_eq!(ann.role, Some(RelationRole::Implements));
        let ann = parse_relation_annotation("// @relation(BR-001, role=Verifies)").unwrap();
        assert_eq!(ann.role, Some(RelationRole::Verifies));
        let ann = parse_relation_annotation("// @relation(BR-001, role=Custom)").unwrap();
        assert_eq!(ann.role, Some(RelationRole::Other("Custom".to_string())));
    }

    // r[verify ann.multi-uid]
    #[test]
    fn multiple_uids_parsed() {
        let ann =
            parse_relation_annotation("// @relation(BR-001, BR-003, scope=function)").unwrap();
        assert_eq!(ann.uids, vec!["BR-001", "BR-003"]);
    }

    // r[verify ann.span]
    #[test]
    fn span_covers_full_call() {
        let line = "    // @relation(BR-001) more text";
        let ann = parse_relation_annotation(line).unwrap();
        assert_eq!(&line[ann.start..ann.end], "@relation(BR-001)");
    }

    // r[verify ann.non-match]
    #[test]
    fn no_annotation_returns_none() {
        assert!(parse_relation_annotation("// nothing here").is_none());
        assert!(parse_relation_annotation("@relations(BR-001)").is_none());
        assert!(parse_relation_annotation("@relation").is_none());
        assert!(parse_relation_annotation("@relation(").is_none());
        assert!(parse_relation_annotation("@relation()").is_none());
    }

    #[test]
    fn whitespace_tolerant() {
        let ann =
            parse_relation_annotation("// @relation( BR-001 , scope = function , role = Refines )")
                .unwrap();
        assert_eq!(ann.uids, vec!["BR-001"]);
        assert_eq!(ann.scope, RelationScope::Function);
        assert_eq!(ann.role, Some(RelationRole::Refines));
    }

    #[test]
    fn invalid_scope_value_skips_call() {
        // Unknown scope value → parse_inner returns None → outer loop moves on.
        let res = parse_relation_annotation("// @relation(BR-001, scope=bogus)");
        // With our current behaviour, the unknown scope makes that call fail
        // to parse and no subsequent `@relation` exists, so we return None.
        assert!(res.is_none());
    }
}
