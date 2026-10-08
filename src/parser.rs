// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Recursive-descent parser over the line stream produced by [`lexer`].

use std::collections::BTreeMap;
use std::iter::Peekable;

use crate::ast::{
    Document, DocumentChild, DocumentFromFile, Field, FieldValue, Node, Relation, Section, Span,
};
use crate::error::{ParseError, ParseErrorKind};
use crate::lexer::{self, classify, is_blank, Line, LineKind};

type Lines<'a> = Peekable<lexer::Lines<'a>>;

fn error_at(kind: ParseErrorKind, line: &Line<'_>) -> ParseError {
    ParseError::new(kind, line.line_no, 1, line.start)
}

// r[impl doc.header]
// r[impl doc.fields]
pub(crate) fn parse_document(input: &str) -> Result<Document, ParseError> {
    // Upstream strips a UTF-8 byte-order mark before parsing.
    let bom_len = if input.starts_with('\u{feff}') { 3 } else { 0 };
    let mut lines = lexer::lines_from(input, bom_len).peekable();

    skip_blank(&mut lines);
    let doc_header_line = match lines.peek() {
        Some(l) => *l,
        None => {
            return Err(ParseError::new(
                ParseErrorKind::MissingDocumentBlock,
                1,
                1,
                0,
            ))
        }
    };
    if !matches!(
        classify(&doc_header_line),
        LineKind::BlockHeader { tag: "DOCUMENT" }
    ) {
        return Err(error_at(
            ParseErrorKind::MissingDocumentBlock,
            &doc_header_line,
        ));
    }
    lines.next();

    let mut title = None;
    let mut uid = None;
    let mut version = None;
    let mut date = None;
    let mut classification = None;
    let mut prefix = None;
    let mut options: BTreeMap<String, String> = BTreeMap::new();
    let mut metadata: BTreeMap<String, String> = BTreeMap::new();

    // [DOCUMENT] header fields run until the first block boundary.
    while let Some(&line) = lines.peek() {
        match classify(&line) {
            LineKind::BlockHeader { .. }
            | LineKind::DoubleBlockHeader { .. }
            | LineKind::BlockClose { .. }
            | LineKind::LegacyClose { .. } => break,
            LineKind::Field { name, value } => {
                let slot = match name {
                    "TITLE" => Some(&mut title),
                    "UID" => Some(&mut uid),
                    "VERSION" => Some(&mut version),
                    "DATE" => Some(&mut date),
                    "CLASSIFICATION" => Some(&mut classification),
                    // r[impl doc.req-prefix]
                    "PREFIX" | "REQ_PREFIX" => Some(&mut prefix),
                    // Other header fields (MID, ROOT, …) are ignored for
                    // forward compatibility with newer .sdoc grammars.
                    _ => None,
                };
                if let Some(slot) = slot {
                    *slot = Some(value.to_string());
                }
                lines.next();
            }
            LineKind::EmptyField { name: "OPTIONS" } => {
                // r[impl doc.options]
                lines.next();
                read_indented_subblock(&mut lines, &mut options);
            }
            LineKind::EmptyField { name: "METADATA" } => {
                // r[impl doc.metadata]
                lines.next();
                read_metadata(&mut lines, &mut metadata);
            }
            LineKind::EmptyField { name: _ } => {
                // Other structured header blocks (VIEWS:, …): consume the
                // indented body and discard it.
                lines.next();
                let _ = read_indented_block_text(&mut lines, line);
            }
            LineKind::HeredocOpen { name: _ } => {
                lines.next();
                let _ = read_heredoc_body(&mut lines, line)?;
            }
            LineKind::IndentedField { .. } => {
                lines.next();
            }
            LineKind::Other if is_blank(&line) => {
                lines.next();
            }
            LineKind::HeredocClose | LineKind::Other => {
                return Err(error_at(ParseErrorKind::UnexpectedContent, &line));
            }
        }
    }

    let mut grammar_import = None;
    let (body, _) = parse_children(&mut lines, None, &mut grammar_import)?;

    Ok(Document {
        title,
        uid,
        version,
        date,
        classification,
        prefix,
        options,
        metadata,
        grammar_import,
        body,
        span: Span::new(0, input.len(), 1, 1),
    })
}

/// The block whose children [`parse_children`] is currently reading.
struct OpenBlock<'a> {
    tag: &'a str,
    /// `[TAG]` … `[/TAG]` (legacy) rather than `[[TAG]]` … `[[/TAG]]`.
    legacy: bool,
    header: Line<'a>,
}

fn display_close(tag: &str, legacy: bool) -> String {
    if legacy {
        format!("[/{tag}]")
    } else {
        format!("[[/{tag}]]")
    }
}

/// Parse a sequence of sections, nodes and includes. With `open` set, stop
/// after consuming the matching closing tag and return its line; otherwise
/// run to EOF.
fn parse_children<'a>(
    lines: &mut Lines<'a>,
    open: Option<&OpenBlock<'a>>,
    grammar_import: &mut Option<String>,
) -> Result<(Vec<DocumentChild>, Option<Line<'a>>), ParseError> {
    let mut children = Vec::new();

    loop {
        let Some(&line) = lines.peek() else {
            return match open {
                Some(o) => {
                    let tag = if o.legacy {
                        format!("[{}]", o.tag)
                    } else {
                        format!("[[{}]]", o.tag)
                    };
                    Err(error_at(ParseErrorKind::UnclosedBlock { tag }, &o.header))
                }
                None => Ok((children, None)),
            };
        };

        match classify(&line) {
            LineKind::Other if is_blank(&line) => {
                lines.next();
            }
            LineKind::BlockHeader { tag: "DOCUMENT" } => {
                return Err(error_at(ParseErrorKind::UnexpectedContent, &line));
            }
            LineKind::BlockHeader { tag: "GRAMMAR" } => {
                lines.next();
                if let Some(path) = read_grammar_block(lines) {
                    *grammar_import = Some(path);
                }
            }
            LineKind::BlockHeader {
                tag: "DOCUMENT_FROM_FILE",
            } => {
                // r[impl doc.from-file]
                lines.next();
                let (fields, _, end) = read_fields(lines, line)?;
                let file = fields
                    .iter()
                    .find(|f| f.name == "FILE")
                    .map(|f| f.value.text().to_string())
                    .unwrap_or_default();
                children.push(DocumentChild::DocumentFromFile(DocumentFromFile {
                    file,
                    span: Span::new(line.start, end, line.line_no, 1),
                }));
            }
            LineKind::BlockHeader { tag: "SECTION" } => {
                // r[impl sect.legacy]
                lines.next();
                let section = read_section(lines, line, true, grammar_import)?;
                children.push(DocumentChild::Section(section));
            }
            LineKind::DoubleBlockHeader { tag: "SECTION" } => {
                lines.next();
                let section = read_section(lines, line, false, grammar_import)?;
                children.push(DocumentChild::Section(section));
            }
            LineKind::BlockHeader { tag } => {
                // r[impl node.any-tag]
                lines.next();
                let (fields, relations, end) = read_fields(lines, line)?;
                children.push(DocumentChild::Node(Node {
                    node_type: tag.to_string(),
                    fields,
                    relations,
                    composite: false,
                    children: Vec::new(),
                    span: Span::new(line.start, end, line.line_no, 1),
                }));
            }
            LineKind::DoubleBlockHeader { tag } => {
                // r[impl node.composite]
                lines.next();
                let (fields, relations, _) = read_fields(lines, line)?;
                let block = OpenBlock {
                    tag,
                    legacy: false,
                    header: line,
                };
                let (nested, close) = parse_children(lines, Some(&block), grammar_import)?;
                let end = close.map_or(line.end, |c| c.end);
                children.push(DocumentChild::Node(Node {
                    node_type: tag.to_string(),
                    fields,
                    relations,
                    composite: true,
                    children: nested,
                    span: Span::new(line.start, end, line.line_no, 1),
                }));
            }
            kind @ (LineKind::BlockClose { tag } | LineKind::LegacyClose { tag }) => {
                // r[impl sect.open-close]
                let legacy = matches!(kind, LineKind::LegacyClose { .. });
                return match open {
                    Some(o) if o.tag == tag && o.legacy == legacy => {
                        lines.next();
                        Ok((children, Some(line)))
                    }
                    Some(o) => Err(error_at(
                        ParseErrorKind::MismatchedClose {
                            expected: display_close(o.tag, o.legacy),
                            found: display_close(tag, legacy),
                        },
                        &line,
                    )),
                    None => Err(error_at(
                        ParseErrorKind::UnmatchedClose {
                            tag: display_close(tag, legacy),
                        },
                        &line,
                    )),
                };
            }
            _ => {
                return Err(error_at(ParseErrorKind::UnexpectedContent, &line));
            }
        }
    }
}

/// Read a section (header line already consumed) through its closing tag.
// r[impl sect.title]
// r[impl sect.nesting]
fn read_section<'a>(
    lines: &mut Lines<'a>,
    header: Line<'a>,
    legacy: bool,
    grammar_import: &mut Option<String>,
) -> Result<Section, ParseError> {
    let (fields, _, _) = read_fields(lines, header)?;
    let title = fields
        .iter()
        .find(|f| f.name == "TITLE")
        .map(|f| f.value.text().to_string())
        .ok_or_else(|| error_at(ParseErrorKind::MissingSectionTitle, &header))?;
    let block = OpenBlock {
        tag: "SECTION",
        legacy,
        header,
    };
    let (children, close) = parse_children(lines, Some(&block), grammar_import)?;
    let end = close.map_or(header.end, |c| c.end);
    Ok(Section {
        title,
        fields,
        children,
        span: Span::new(header.start, end, header.line_no, 1),
    })
}

/// Read the fields of a block (header line already consumed) up to the next
/// block boundary. Returns the fields, the parsed `RELATIONS:` entries, and
/// the end offset of the last line that belonged to the block.
// r[impl req.fields]
// r[impl req.field-order]
// r[impl req.uid-verbatim]
fn read_fields<'a>(
    lines: &mut Lines<'a>,
    header: Line<'a>,
) -> Result<(Vec<Field>, Vec<Relation>, usize), ParseError> {
    let mut fields: Vec<Field> = Vec::new();
    let mut relations: Vec<Relation> = Vec::new();
    let mut end_offset = header.end;

    while let Some(&line) = lines.peek() {
        match classify(&line) {
            LineKind::BlockHeader { .. }
            | LineKind::DoubleBlockHeader { .. }
            | LineKind::BlockClose { .. }
            | LineKind::LegacyClose { .. } => break,
            LineKind::Field { name, value } => {
                let value_col = compute_value_column(line.text, name);
                let value_span = Span::new(
                    line.start + value_col.saturating_sub(1) as usize,
                    line.end,
                    line.line_no,
                    value_col,
                );
                fields.push(Field {
                    name: name.to_string(),
                    value: FieldValue::SingleLine {
                        text: value.to_string(),
                        span: value_span,
                    },
                    span: Span::new(line.start, line.end, line.line_no, 1),
                });
                end_offset = line.end;
                lines.next();
            }
            LineKind::HeredocOpen { name } => {
                lines.next();
                let (text, body_span, close_end) = read_heredoc_body(lines, line)?;
                fields.push(Field {
                    name: name.to_string(),
                    value: FieldValue::Heredoc {
                        text,
                        span: body_span,
                    },
                    span: Span::new(line.start, close_end, line.line_no, 1),
                });
                end_offset = close_end;
            }
            LineKind::EmptyField { name: "RELATIONS" } => {
                // r[impl node.relations]
                lines.next();
                let (mut parsed, end) = read_relations(lines, line)?;
                relations.append(&mut parsed);
                end_offset = end;
            }
            LineKind::EmptyField { name } => {
                // `KEY:` with no value. Not valid upstream, but capture any
                // indented body verbatim rather than failing.
                lines.next();
                let (text, body_span) = read_indented_block_text(lines, line);
                let end = body_span.end.max(line.end);
                fields.push(Field {
                    name: name.to_string(),
                    value: FieldValue::Heredoc {
                        text,
                        span: body_span,
                    },
                    span: Span::new(line.start, end, line.line_no, 1),
                });
                end_offset = end;
            }
            LineKind::IndentedField { .. } => {
                // Stray indented line; consume and ignore.
                end_offset = line.end;
                lines.next();
            }
            LineKind::Other if is_blank(&line) => {
                lines.next();
            }
            LineKind::HeredocClose | LineKind::Other => {
                return Err(error_at(ParseErrorKind::UnexpectedContent, &line));
            }
        }
    }

    Ok((fields, relations, end_offset))
}

/// Read the `- TYPE: …` list that follows a `RELATIONS:` line.
fn read_relations<'a>(
    lines: &mut Lines<'a>,
    opener: Line<'a>,
) -> Result<(Vec<Relation>, usize), ParseError> {
    let mut relations: Vec<Relation> = Vec::new();
    let mut end_offset = opener.end;

    while let Some(&line) = lines.peek() {
        if let Some(entry) = line.text.strip_prefix("- ") {
            let (key, value) = entry
                .split_once(':')
                .ok_or_else(|| error_at(ParseErrorKind::MalformedRelation, &line))?;
            if key.trim() != "TYPE" {
                return Err(error_at(ParseErrorKind::MalformedRelation, &line));
            }
            relations.push(Relation {
                relation_type: value.trim().to_string(),
                role: None,
                properties: Vec::new(),
                span: Span::new(line.start, line.end, line.line_no, 1),
            });
        } else if let LineKind::IndentedField { name, value, .. } = classify(&line) {
            let relation = relations
                .last_mut()
                .ok_or_else(|| error_at(ParseErrorKind::MalformedRelation, &line))?;
            if name == "ROLE" {
                relation.role = Some(value.to_string());
            } else {
                relation
                    .properties
                    .push((name.to_string(), value.to_string()));
            }
            relation.span.end = line.end;
        } else {
            break;
        }
        end_offset = line.end;
        lines.next();
    }

    Ok((relations, end_offset))
}

/// Read the body of a `[GRAMMAR]` block. Returns the `IMPORT_FROM_FILE:`
/// path if one is found.
///
/// StrictDoc `[GRAMMAR]` blocks come in two shapes: a single
/// `IMPORT_FROM_FILE:` reference to an external `.sgra` file, or an inline
/// grammar declaration that uses a YAML-like syntax (`ELEMENTS:`,
/// `FIELDS:`, leading `- ` list items, deep indentation). The grammar is
/// treated as opaque metadata: we capture the import path when present and
/// skip everything else until the next block boundary.
// r[impl doc.grammar-import]
fn read_grammar_block(lines: &mut Lines<'_>) -> Option<String> {
    let mut import_path = None;
    while let Some(&line) = lines.peek() {
        match classify(&line) {
            LineKind::BlockHeader { .. }
            | LineKind::DoubleBlockHeader { .. }
            | LineKind::BlockClose { .. }
            | LineKind::LegacyClose { .. } => break,
            LineKind::Field {
                name: "IMPORT_FROM_FILE",
                value,
            } => {
                import_path = Some(value.to_string());
            }
            _ => {}
        }
        lines.next();
    }
    import_path
}

/// Read the body of a heredoc that has already had its opening `KEY: >>>`
/// line consumed. Returns the body text (every line up to the closing
/// `<<<`, joined with `\n`), the body's span, and the end byte offset of
/// the closing `<<<` line.
// r[impl req.heredoc-verbatim]
fn read_heredoc_body<'a>(
    lines: &mut Lines<'a>,
    opener: Line<'a>,
) -> Result<(String, Span, usize), ParseError> {
    let mut content_lines: Vec<Line<'_>> = Vec::new();
    let close = loop {
        let line = lines
            .next()
            .ok_or_else(|| error_at(ParseErrorKind::UnterminatedHeredoc, &opener))?;
        if matches!(classify(&line), LineKind::HeredocClose) {
            break line;
        }
        content_lines.push(line);
    };

    let text = content_lines
        .iter()
        .map(|l| l.text)
        .collect::<Vec<_>>()
        .join("\n");

    let span = match (content_lines.first(), content_lines.last()) {
        (Some(first), Some(last)) => Span::new(first.start, last.end, first.line_no, 1),
        _ => Span::new(close.start, close.start, close.line_no, 1),
    };
    Ok((text, span, close.end))
}

/// Read indented `KEY: value` lines into the provided map. Stops at the
/// first non-indented or block-boundary line.
fn read_indented_subblock(lines: &mut Lines<'_>, out: &mut BTreeMap<String, String>) {
    while let Some(&line) = lines.peek() {
        match classify(&line) {
            LineKind::IndentedField { name, value, .. } => {
                out.insert(name.to_string(), value.to_string());
                lines.next();
            }
            LineKind::Other if is_blank(&line) => {
                lines.next();
            }
            _ => break,
        }
    }
}

/// Read the indented `key: value` lines of a `METADATA:` block. Keys are
/// free-form (upstream allows lowercase), so this doesn't go through
/// [`classify`].
fn read_metadata(lines: &mut Lines<'_>, out: &mut BTreeMap<String, String>) {
    while let Some(&line) = lines.peek() {
        if is_blank(&line) {
            lines.next();
            continue;
        }
        if !line.text.starts_with([' ', '\t']) {
            break;
        }
        if let Some((key, value)) = line.text.split_once(':') {
            out.insert(key.trim().to_string(), value.trim().to_string());
        }
        lines.next();
    }
}

/// Read an indented (or `- `-prefixed YAML-list) block following an empty
/// `KEY:` line as a single verbatim text blob.
fn read_indented_block_text<'a>(lines: &mut Lines<'a>, opener: Line<'a>) -> (String, Span) {
    let mut collected: Vec<&str> = Vec::new();
    let mut start_offset = opener.end;
    let mut end_offset = opener.end;
    let mut start_line = opener.line_no + 1;

    while let Some(&line) = lines.peek() {
        if is_blank(&line) {
            lines.next();
            continue;
        }
        if !(line.text.starts_with([' ', '\t']) || line.text.starts_with("- ")) {
            break;
        }
        if collected.is_empty() {
            start_offset = line.start;
            start_line = line.line_no;
        }
        end_offset = line.end;
        collected.push(line.text);
        lines.next();
    }

    (
        collected.join("\n"),
        Span::new(start_offset, end_offset, start_line, 1),
    )
}

/// Compute the 1-indexed column where a field's value begins, given the
/// full line text and the field name. Looks for `NAME:`, skips the colon
/// and any spaces.
fn compute_value_column(line_text: &str, name: &str) -> u32 {
    let lead_ws = line_text.len() - line_text.trim_start().len();
    let after_lead = &line_text[lead_ws..];
    if !after_lead.starts_with(name) {
        return 1;
    }
    let after_name = &after_lead[name.len()..];
    let after_colon = match after_name.strip_prefix(':') {
        Some(s) => s,
        None => return 1,
    };
    let after_ws = after_colon.trim_start();
    let consumed = lead_ws + name.len() + 1 + (after_colon.len() - after_ws.len());
    (consumed + 1) as u32
}

fn skip_blank(lines: &mut Lines<'_>) {
    while lines.next_if(is_blank).is_some() {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_document_with_title() {
        let input = "[DOCUMENT]\nTITLE: Hello\nUID: TEST-DOC\n";
        let doc = parse_document(input).expect("must parse");
        assert_eq!(doc.title.as_deref(), Some("Hello"));
        assert_eq!(doc.uid.as_deref(), Some("TEST-DOC"));
        assert!(doc.body.is_empty());
    }

    #[test]
    fn parse_options_subblock() {
        let input = "[DOCUMENT]\nTITLE: T\nOPTIONS:\n  MARKUP: Text\n  AUTO_LEVELS: On\n";
        let doc = parse_document(input).expect("must parse");
        assert_eq!(doc.options.get("MARKUP"), Some(&"Text".to_string()));
        assert_eq!(doc.options.get("AUTO_LEVELS"), Some(&"On".to_string()));
    }

    #[test]
    fn parse_grammar_import() {
        let input = "[DOCUMENT]\nTITLE: T\n\n[GRAMMAR]\nIMPORT_FROM_FILE: grammar.sgra\n";
        let doc = parse_document(input).expect("must parse");
        assert_eq!(doc.grammar_import.as_deref(), Some("grammar.sgra"));
    }

    #[test]
    fn missing_document_block() {
        let err = parse_document("[REQUIREMENT]\nUID: X\n").unwrap_err();
        assert_eq!(err.kind, ParseErrorKind::MissingDocumentBlock);
        assert_eq!(err.line, 1);
    }

    #[test]
    fn byte_order_mark_is_skipped() {
        let input = "\u{feff}[DOCUMENT]\nTITLE: T\n";
        let doc = parse_document(input).expect("must parse");
        assert_eq!(doc.title.as_deref(), Some("T"));
    }
}
