// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Parser for StrictDoc-style `@relation(...)` source-comment annotations.
//!
//! These annotations live in source-code comments and link implementation
//! sites to requirements. Examples:
//!
//! ```text
//! // @relation(BR-001, scope=function)
//! // @relation(BR-001, scope=function, role=Implementation)
//! // @relation(BR-001, BR-003, scope=range_start)
//! /** @relation{BR-001, scope=class} */
//! ```
//!
//! The accepted syntax follows upstream StrictDoc's marker grammar:
//!
//! ```text
//! @relation( UID (", " UID)* [", scope=" SCOPE] [", role=" ROLE] )
//! ```
//!
//! where `(`/`)` may also be `{`/`}`, `UID` is `[A-Za-z][A-Za-z0-9_/.-]+`,
//! `SCOPE` is one of `file`, `class`, `function`, `line`, `range_start`,
//! `range_end`, and `ROLE` is `[A-Za-z0-9_]+`. Whitespace (including line
//! breaks followed by a comment leader such as `*` or `//`) may follow the
//! opening brace, each comma, and precede the closing brace.

use std::fmt;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RelationAnnotation {
    /// One or more UIDs the annotation refers to (preserved verbatim).
    pub uids: Vec<String>,
    /// Annotation scope, or `None` if the marker has no `scope=` argument.
    ///
    /// Upstream StrictDoc only accepts a missing scope where the language
    /// reader supplies a default (e.g. Rust doc comments); choosing that
    /// default is up to the caller.
    pub scope: Option<RelationScope>,
    /// The `role=` value, verbatim. Upstream treats roles as free-form; see
    /// [`RelationAnnotation::role_kind`] for a classification.
    pub role: Option<String>,
    /// Byte offsets within the input that correspond to the matched
    /// `@relation(...)` call, inclusive of `@relation` and the closing
    /// brace.
    pub start: usize,
    pub end: usize,
}

impl RelationAnnotation {
    /// Classify [`role`](Self::role) with [`RelationRole::classify`].
    pub fn role_kind(&self) -> Option<RelationRole> {
        self.role.as_deref().map(RelationRole::classify)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[non_exhaustive]
pub enum RelationScope {
    File,
    Class,
    Function,
    Line,
    RangeStart,
    RangeEnd,
}

impl RelationScope {
    /// Parse a `scope=` keyword value (`"function"`, `"range_start"`, …).
    pub fn from_keyword(s: &str) -> Option<Self> {
        Some(match s {
            "file" => Self::File,
            "class" => Self::Class,
            "function" => Self::Function,
            "line" => Self::Line,
            "range_start" => Self::RangeStart,
            "range_end" => Self::RangeEnd,
            _ => return None,
        })
    }

    /// The `scope=` keyword value for this scope.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Class => "class",
            Self::Function => "function",
            Self::Line => "line",
            Self::RangeStart => "range_start",
            Self::RangeEnd => "range_end",
        }
    }
}

impl fmt::Display for RelationScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Semantic classification of a free-form `role=` value.
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

impl RelationRole {
    /// Classify a role value, ignoring ASCII case. Both the verb and the
    /// noun spelling are recognised, since upstream StrictDoc documents
    /// both (`Implementation` or `Implements`, `Test` or `Verifies`):
    ///
    /// - `Implements`: `Implements`, `Implement`, `Implementation`, `Impl`
    /// - `Verifies`: `Verifies`, `Verify`, `Verification`, `Test`, `Tests`
    /// - `Refines`: `Refines`, `Refine`, `Refinement`
    // r[impl ann.role-kind]
    pub fn classify(role: &str) -> Self {
        const IMPLEMENTS: &[&str] = &["implements", "implement", "implementation", "impl"];
        const VERIFIES: &[&str] = &["verifies", "verify", "verification", "test", "tests"];
        const REFINES: &[&str] = &["refines", "refine", "refinement"];
        let is = |names: &[&str]| names.iter().any(|n| n.eq_ignore_ascii_case(role));
        if is(IMPLEMENTS) {
            Self::Implements
        } else if is(VERIFIES) {
            Self::Verifies
        } else if is(REFINES) {
            Self::Refines
        } else {
            Self::Other(role.to_string())
        }
    }
}

/// A `@relation(` / `@relation{` call that does not follow the marker
/// grammar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotationError {
    pub kind: AnnotationErrorKind,
    /// Byte offset of the `@relation` marker.
    pub start: usize,
    /// Byte offset at which the problem was detected.
    pub offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AnnotationErrorKind {
    /// The input ended before the closing `)` / `}`.
    Unterminated,
    /// The call has no UID before its keyword arguments.
    MissingUid,
    /// A UID token does not match `[A-Za-z][A-Za-z0-9_/.-]+`.
    InvalidUid(String),
    /// The same UID appears twice in one call.
    DuplicateUid(String),
    /// Unknown `scope=` value.
    InvalidScope(String),
    /// `role=` value is empty or not `[A-Za-z0-9_]+`.
    InvalidRole(String),
    /// An unknown `key=value` argument, a repeated keyword, or an argument
    /// out of order (UIDs, then `scope=`, then `role=`).
    UnexpectedArgument(String),
    /// Anything else, e.g. a missing space after a comma.
    Syntax(&'static str),
}

impl fmt::Display for AnnotationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid @relation marker: ")?;
        match &self.kind {
            AnnotationErrorKind::Unterminated => f.write_str("missing closing `)`"),
            AnnotationErrorKind::MissingUid => f.write_str("no UID given"),
            AnnotationErrorKind::InvalidUid(s) => write!(f, "invalid UID `{s}`"),
            AnnotationErrorKind::DuplicateUid(s) => write!(f, "duplicate UID `{s}`"),
            AnnotationErrorKind::InvalidScope(s) => write!(
                f,
                "invalid scope `{s}` (expected file, class, function, line, range_start or range_end)"
            ),
            AnnotationErrorKind::InvalidRole(s) => write!(f, "invalid role `{s}`"),
            AnnotationErrorKind::UnexpectedArgument(s) => write!(
                f,
                "unexpected argument `{s}` (expected UIDs, then scope=, then role=)"
            ),
            AnnotationErrorKind::Syntax(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for AnnotationError {}

const MARKER: &str = "@relation";

/// Characters that may make up a comment leader (`//`, `#`, `*`, `--`,
/// `<!--`, `(*`, `..`, `"""`, …).
const LEADER_CHARS: &[u8] = b"/*!#-;%\"'<({.";

/// Find every `@relation(...)` / `@relation{...}` call in `text`.
///
/// As in upstream StrictDoc, a marker only counts when it starts a comment
/// line: everything before it on its line must be whitespace, or end in a
/// comment leader such as `//`, `#`, `*` or `--`. Markers mentioned
/// mid-sentence (`see @relation(X) for details`) are ignored, while a
/// trailing comment (`foo(); // @relation(X, scope=line)`) is accepted.
///
/// Calls that are recognised as markers but don't follow the grammar are
/// returned as errors rather than skipped.
// r[impl ann.comment-start]
// r[impl ann.errors]
pub fn find_relation_annotations(text: &str) -> Vec<Result<RelationAnnotation, AnnotationError>> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut pos = 0;
    let mut last_end = None;
    while let Some(off) = text[pos..].find(MARKER) {
        let at = pos + off;
        let open = at + MARKER.len();
        // r[impl ann.brace-form]
        if !matches!(bytes.get(open), Some(b'(' | b'{')) || !starts_comment_line(text, at, last_end)
        {
            pos = open;
            continue;
        }
        match parse_marker(text, at, open + 1) {
            Ok(ann) => {
                pos = ann.end;
                last_end = Some(ann.end);
                out.push(Ok(ann));
            }
            Err(e) => {
                pos = open + 1;
                out.push(Err(e));
            }
        }
    }
    out
}

/// Return the first well-formed `@relation(...)` call in `input`, or
/// `None` if there is none. Use [`find_relation_annotations`] to see every
/// call, including malformed ones.
// r[impl ann.minimal]
// r[impl ann.non-match]
pub fn parse_relation_annotation(input: &str) -> Option<RelationAnnotation> {
    find_relation_annotations(input)
        .into_iter()
        .find_map(Result::ok)
}

/// True if the marker at `at` is preceded on its line only by whitespace,
/// a comment leader, or (directly) by another marker ending at `last_end`.
fn starts_comment_line(text: &str, at: usize, last_end: Option<usize>) -> bool {
    let line_start = text[..at].rfind('\n').map_or(0, |i| i + 1);
    if let Some(end) = last_end {
        if end >= line_start && text[end..at].trim().is_empty() {
            return true;
        }
    }
    let prefix = text[line_start..at].trim_end();
    match prefix.rsplit(char::is_whitespace).next() {
        None | Some("") => true,
        Some(token) => token.bytes().all(|b| LEADER_CHARS.contains(&b)),
    }
}

/// Skip whitespace. A line break may be followed by indentation and a
/// comment leader, so markers can span lines of a block or line comment.
fn skip_ws(bytes: &[u8], mut p: usize) -> usize {
    while let Some(&b) = bytes.get(p) {
        match b {
            b' ' | b'\t' | b'\r' => p += 1,
            b'\n' => {
                p += 1;
                while matches!(bytes.get(p), Some(b' ' | b'\t')) {
                    p += 1;
                }
                while bytes.get(p).is_some_and(|b| LEADER_CHARS.contains(b)) {
                    p += 1;
                }
            }
            _ => break,
        }
    }
    p
}

/// Read a run of characters that can form an argument: UID characters,
/// `=` and `_`.
fn read_word(text: &str, p: usize) -> (&str, usize) {
    let len = text.as_bytes()[p..]
        .iter()
        .take_while(|b| b.is_ascii_alphanumeric() || b"_/.-=".contains(b))
        .count();
    (&text[p..p + len], p + len)
}

// r[impl ann.uid-syntax]
fn is_uid(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() >= 2
        && bytes[0].is_ascii_alphabetic()
        && bytes[1..]
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || b"_/.-".contains(b))
}

fn is_role(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Parse the arguments of a marker whose opening brace ends just before
/// `p`.
// r[impl ann.scope]
// r[impl ann.role]
// r[impl ann.multi-uid]
// r[impl ann.span]
fn parse_marker(text: &str, at: usize, p: usize) -> Result<RelationAnnotation, AnnotationError> {
    let bytes = text.as_bytes();
    let err = |kind, offset| AnnotationError {
        kind,
        start: at,
        offset,
    };

    let mut uids: Vec<String> = Vec::new();
    let mut scope: Option<RelationScope> = None;
    let mut role: Option<String> = None;

    let p = skip_ws(bytes, p);
    let (word, mut p_end) = read_word(text, p);
    if word.is_empty() {
        return Err(match bytes.get(p) {
            None => err(AnnotationErrorKind::Unterminated, p),
            Some(b')' | b'}') => err(AnnotationErrorKind::MissingUid, p),
            Some(_) => err(AnnotationErrorKind::InvalidUid(next_token(text, p)), p),
        });
    }
    if word.contains('=') {
        return Err(err(AnnotationErrorKind::MissingUid, p));
    }
    if !is_uid(word) {
        return Err(err(AnnotationErrorKind::InvalidUid(word.to_string()), p));
    }
    uids.push(word.to_string());

    loop {
        let q = skip_ws(bytes, p_end);
        match bytes.get(q) {
            Some(b')' | b'}') => {
                return Ok(RelationAnnotation {
                    uids,
                    scope,
                    role,
                    start: at,
                    end: q + 1,
                });
            }
            Some(b',') if q == p_end => {
                let r = skip_ws(bytes, q + 1);
                if r >= bytes.len() {
                    return Err(err(AnnotationErrorKind::Unterminated, r));
                }
                if r == q + 1 {
                    return Err(err(
                        AnnotationErrorKind::Syntax("expected a space after `,`"),
                        r,
                    ));
                }
                let (word, end) = read_word(text, r);
                if let Some(value) = word.strip_prefix("scope=") {
                    if scope.is_some() || role.is_some() {
                        return Err(err(
                            AnnotationErrorKind::UnexpectedArgument(word.to_string()),
                            r,
                        ));
                    }
                    scope = Some(RelationScope::from_keyword(value).ok_or_else(|| {
                        err(AnnotationErrorKind::InvalidScope(value.to_string()), r)
                    })?);
                } else if let Some(value) = word.strip_prefix("role=") {
                    if role.is_some() {
                        return Err(err(
                            AnnotationErrorKind::UnexpectedArgument(word.to_string()),
                            r,
                        ));
                    }
                    if !is_role(value) {
                        return Err(err(AnnotationErrorKind::InvalidRole(value.to_string()), r));
                    }
                    role = Some(value.to_string());
                } else if word.contains('=') || scope.is_some() || role.is_some() {
                    return Err(err(
                        AnnotationErrorKind::UnexpectedArgument(word.to_string()),
                        r,
                    ));
                } else if word.is_empty() {
                    return Err(match bytes.get(r) {
                        None => err(AnnotationErrorKind::Unterminated, r),
                        Some(_) => err(AnnotationErrorKind::InvalidUid(next_token(text, r)), r),
                    });
                } else if !is_uid(word) {
                    return Err(err(AnnotationErrorKind::InvalidUid(word.to_string()), r));
                } else if uids.iter().any(|u| u == word) {
                    return Err(err(AnnotationErrorKind::DuplicateUid(word.to_string()), r));
                } else {
                    uids.push(word.to_string());
                }
                p_end = end;
            }
            Some(b',') => {
                return Err(err(
                    AnnotationErrorKind::Syntax("unexpected whitespace before `,`"),
                    p_end,
                ));
            }
            None => return Err(err(AnnotationErrorKind::Unterminated, q)),
            Some(_) => {
                return Err(err(
                    AnnotationErrorKind::Syntax("expected `,` or a closing `)`"),
                    q,
                ));
            }
        }
    }
}

/// The non-whitespace run starting at `p`, for error messages.
fn next_token(text: &str, p: usize) -> String {
    text[p..]
        .split(|c: char| c.is_whitespace() || c == ',' || c == ')' || c == '}')
        .next()
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(input: &str) -> AnnotationErrorKind {
        match find_relation_annotations(input).into_iter().next() {
            Some(Err(e)) => e.kind,
            other => panic!("expected an error for {input:?}, got {other:?}"),
        }
    }

    // r[verify ann.minimal]
    #[test]
    fn minimal_form_has_no_scope_or_role() {
        let ann = parse_relation_annotation("@relation(BR-001)").expect("recognised");
        assert_eq!(ann.uids, vec!["BR-001"]);
        assert_eq!(ann.scope, None);
        assert_eq!(ann.role, None);
    }

    // r[verify ann.scope]
    #[test]
    fn every_scope_parsed() {
        for (kw, scope) in [
            ("file", RelationScope::File),
            ("class", RelationScope::Class),
            ("function", RelationScope::Function),
            ("line", RelationScope::Line),
            ("range_start", RelationScope::RangeStart),
            ("range_end", RelationScope::RangeEnd),
        ] {
            let ann = parse_relation_annotation(&format!("// @relation(BR-001, scope={kw})"))
                .unwrap_or_else(|| panic!("scope={kw}"));
            assert_eq!(ann.scope, Some(scope));
            assert_eq!(scope.as_str(), kw);
        }
    }

    // r[verify ann.role]
    #[test]
    fn role_is_kept_verbatim() {
        let ann =
            parse_relation_annotation("// @relation(BR-001, scope=function, role=Implementation)")
                .unwrap();
        assert_eq!(ann.role.as_deref(), Some("Implementation"));
        let ann = parse_relation_annotation("// @relation(BR-001, role=Custom)").unwrap();
        assert_eq!(ann.role.as_deref(), Some("Custom"));
        assert_eq!(ann.scope, None);
    }

    // r[verify ann.role-kind]
    #[test]
    fn role_kind_accepts_verb_and_noun_forms() {
        for r in ["Implements", "Implementation", "implementation", "Impl"] {
            assert_eq!(RelationRole::classify(r), RelationRole::Implements, "{r}");
        }
        for r in ["Verifies", "Verification", "Test", "tests"] {
            assert_eq!(RelationRole::classify(r), RelationRole::Verifies, "{r}");
        }
        for r in ["Refines", "Refinement"] {
            assert_eq!(RelationRole::classify(r), RelationRole::Refines, "{r}");
        }
        assert_eq!(
            RelationRole::classify("Mitigates"),
            RelationRole::Other("Mitigates".to_string())
        );
        let ann = parse_relation_annotation("// @relation(BR-001, scope=line, role=Test)").unwrap();
        assert_eq!(ann.role_kind(), Some(RelationRole::Verifies));
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

    // r[verify ann.brace-form]
    #[test]
    fn brace_form_parsed() {
        let line = " * @relation{REQ-1, scope=function}";
        let ann = parse_relation_annotation(line).unwrap();
        assert_eq!(ann.uids, vec!["REQ-1"]);
        assert_eq!(
            &line[ann.start..ann.end],
            "@relation{REQ-1, scope=function}"
        );
    }

    // r[verify ann.non-match]
    #[test]
    fn no_annotation_returns_none() {
        assert!(parse_relation_annotation("// nothing here").is_none());
        assert!(find_relation_annotations("@relations(BR-001)").is_empty());
        assert!(find_relation_annotations("@relation").is_empty());
        assert!(find_relation_annotations("@relation (BR-001)").is_empty());
        assert!(find_relation_annotations("// see @relation, no braces").is_empty());
    }

    // r[verify ann.comment-start]
    #[test]
    fn marker_must_start_a_comment_line() {
        for ok in [
            "@relation(REQ-1, scope=function)",
            "   @relation(REQ-1, scope=function)",
            "// @relation(REQ-1, scope=function)",
            "/// @relation(REQ-1, scope=function)",
            "//! @relation(REQ-1, scope=function)",
            "# @relation(REQ-1, scope=function)",
            " * @relation(REQ-1, scope=function)",
            "-- @relation(REQ-1, scope=function)",
            "<!-- @relation(REQ-1, scope=function) -->",
            "foo();  // @relation(REQ-1, scope=line)",
            "#[doc = \" @relation(REQ-1)\"]",
            "Example:\n@relation(REQ-1, scope=function)",
        ] {
            assert!(parse_relation_annotation(ok).is_some(), "{ok:?}");
        }
        for ignored in [
            "see @relation(REQ-1, scope=function) for details",
            "// a marker like ``@relation(REQ-1, scope=function)``",
            "f\"@relation({reqs}, scope=function)\"",
        ] {
            assert!(find_relation_annotations(ignored).is_empty(), "{ignored:?}");
        }
    }

    // r[verify ann.uid-syntax]
    #[test]
    fn uid_syntax_matches_upstream() {
        let ann = parse_relation_annotation("// @relation(REQ.1/a_b-2, scope=file)").unwrap();
        assert_eq!(ann.uids, vec!["REQ.1/a_b-2"]);
        assert_eq!(
            kind("// @relation(R, scope=function)"),
            AnnotationErrorKind::InvalidUid("R".into())
        );
        assert_eq!(
            kind("// @relation(1REQ, scope=function)"),
            AnnotationErrorKind::InvalidUid("1REQ".into())
        );
        assert_eq!(
            kind("// @relation({reqs}, scope=function)"),
            AnnotationErrorKind::InvalidUid("{reqs".into())
        );
    }

    // r[verify ann.errors]
    #[test]
    fn malformed_markers_are_reported() {
        assert_eq!(kind("@relation("), AnnotationErrorKind::Unterminated);
        assert_eq!(kind("@relation(REQ-1,"), AnnotationErrorKind::Unterminated);
        assert_eq!(kind("@relation()"), AnnotationErrorKind::MissingUid);
        assert_eq!(
            kind("@relation(scope=file)"),
            AnnotationErrorKind::MissingUid
        );
        assert_eq!(
            kind("@relation(REQ-1, scope=bogus)"),
            AnnotationErrorKind::InvalidScope("bogus".into())
        );
        assert_eq!(
            kind("@relation(REQ-1, REQ-1, scope=function)"),
            AnnotationErrorKind::DuplicateUid("REQ-1".into())
        );
        assert_eq!(
            kind("@relation(REQ-1, role=Verifies, scope=function)"),
            AnnotationErrorKind::UnexpectedArgument("scope=function".into())
        );
        assert_eq!(
            kind("@relation(REQ-1, scope=file, REQ-2)"),
            AnnotationErrorKind::UnexpectedArgument("REQ-2".into())
        );
        assert_eq!(
            kind("@relation(REQ-1, foo=bar)"),
            AnnotationErrorKind::UnexpectedArgument("foo=bar".into())
        );
        assert!(matches!(
            kind("@relation(REQ-1,scope=function)"),
            AnnotationErrorKind::Syntax(_)
        ));
        assert!(matches!(
            kind("@relation(REQ-1 , scope=function)"),
            AnnotationErrorKind::Syntax(_)
        ));
        // A malformed marker doesn't hide a later valid one.
        let all = find_relation_annotations("@relation(\n// @relation(REQ-2, scope=line)");
        assert_eq!(all.len(), 2);
        assert!(all[0].is_err());
        assert_eq!(all[1].as_ref().unwrap().uids, vec!["REQ-2"]);
    }

    #[test]
    fn whitespace_inside_braces_and_across_lines() {
        let ann = parse_relation_annotation("// @relation( BR-001, scope=function, role=Refines )")
            .unwrap();
        assert_eq!(ann.uids, vec!["BR-001"]);
        assert_eq!(ann.role.as_deref(), Some("Refines"));

        let text = "/**\n * @relation(REQ-1,\n *   REQ-2, scope=function)\n */";
        let ann = parse_relation_annotation(text).unwrap();
        assert_eq!(ann.uids, vec!["REQ-1", "REQ-2"]);
        assert_eq!(ann.scope, Some(RelationScope::Function));
    }

    #[test]
    fn multiple_markers_on_one_line() {
        let all = find_relation_annotations(
            "@relation(REQ-1, scope=range_start) @relation(REQ-2, scope=range_end)",
        );
        assert_eq!(all.len(), 2);
        assert!(all.iter().all(Result::is_ok));
        // Text between the two markers makes the second one prose.
        let all = find_relation_annotations("@relation(REQ-1, scope=file) and @relation(REQ-2)");
        assert_eq!(all.len(), 1);
    }
}
