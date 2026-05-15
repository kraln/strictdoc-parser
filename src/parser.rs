// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Recursive-descent parser over the line stream produced by [`lexer`].

use std::collections::BTreeMap;

use crate::ast::{Document, DocumentChild, Field, FieldValue, Requirement, Section, Span};
use crate::error::{ParseError, ParseErrorKind};
use crate::lexer::{self, classify, is_blank, Line, LineKind};

// r[impl doc.header]
// r[impl doc.fields]
pub(crate) fn parse_document(input: &str) -> Result<Document, ParseError> {
    let mut lines = lexer::lines(input).peekable();

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
    match classify(&doc_header_line) {
        LineKind::BlockHeader { tag: "DOCUMENT" } => {}
        _ => {
            return Err(ParseError::new(
                ParseErrorKind::MissingDocumentBlock,
                doc_header_line.line_no,
                1,
                doc_header_line.start,
            ));
        }
    }
    lines.next();

    let mut title = None;
    let mut uid = None;
    let mut version = None;
    let mut date = None;
    let mut classification = None;
    let mut prefix = None;
    let mut options: BTreeMap<String, String> = BTreeMap::new();
    let mut grammar_import = None;
    let mut body: Vec<DocumentChild> = Vec::new();

    // Read DOCUMENT header fields until we hit a new block boundary.
    while let Some(&line) = lines.peek() {
        let kind = classify(&line);
        match kind {
            LineKind::BlockHeader { tag: "DOCUMENT" } => {
                return Err(ParseError::new(
                    ParseErrorKind::UnexpectedContent,
                    line.line_no,
                    1,
                    line.start,
                ));
            }
            LineKind::BlockHeader { tag: "GRAMMAR" } => {
                lines.next();
                grammar_import = read_grammar_block(&mut lines)?;
            }
            LineKind::BlockHeader { tag: "REQUIREMENT" } => {
                lines.next();
                let req = read_requirement_block(&mut lines, line)?;
                body.push(DocumentChild::Requirement(req));
            }
            LineKind::DoubleBlockHeader { tag: "SECTION" } => {
                lines.next();
                let section = read_section_block(&mut lines, line)?;
                body.push(DocumentChild::Section(section));
            }
            LineKind::BlockClose { tag: _ } => {
                return Err(ParseError::new(
                    ParseErrorKind::UnmatchedSectionClose,
                    line.line_no,
                    1,
                    line.start,
                ));
            }
            LineKind::BlockHeader { tag: _ } => {
                // Unknown single-bracket block (e.g. [TEXT], [FEATURE],
                // user-grammar requirement-like tags). v0.1 consumes the
                // body until the next block boundary; v0.2 may surface
                // these as generic AST nodes.
                lines.next();
                skip_block_body(&mut lines, None);
            }
            LineKind::DoubleBlockHeader { tag } => {
                // Unknown double-bracket block — consume until its
                // matching `[[/TAG]]` close. Handles constructs like
                // `[[COMPOSITE_REQUIREMENT]]` that wrap nested content
                // including [REQUIREMENT] blocks.
                let close_tag = tag.to_string();
                lines.next();
                skip_block_body(&mut lines, Some(&close_tag));
            }
            LineKind::Field { name, value } => {
                match name {
                    "TITLE" => title = Some(value.to_string()),
                    "UID" => uid = Some(value.to_string()),
                    "VERSION" => version = Some(value.to_string()),
                    "DATE" => date = Some(value.to_string()),
                    "CLASSIFICATION" => classification = Some(value.to_string()),
                    "PREFIX" => prefix = Some(value.to_string()),
                    _ => {
                        // Unknown document-header field: silently ignore for
                        // forward compatibility with newer .sdoc grammars.
                    }
                }
                lines.next();
            }
            LineKind::EmptyField { name: "OPTIONS" } => {
                // r[impl doc.options]
                lines.next();
                read_indented_subblock(&mut lines, &mut options);
            }
            LineKind::EmptyField { name: _ } => {
                // Empty document-header field followed by an indented or
                // YAML-list body we don't recognise (e.g. METADATA:,
                // VIEWS:, SPECIAL_FIELDS:). Consume the body tolerantly
                // and discard. v0.2 can surface this as opaque metadata.
                let opener = line;
                lines.next();
                let _ = read_indented_block_text(&mut lines, opener);
            }
            LineKind::HeredocOpen { name: _ } => {
                lines.next();
                let _ = read_heredoc_body(&mut lines, line)?;
                // Heredocs at the document-header level aren't part of the
                // spec; data is captured but discarded.
            }
            LineKind::HeredocClose => {
                return Err(ParseError::new(
                    ParseErrorKind::UnexpectedContent,
                    line.line_no,
                    1,
                    line.start,
                ));
            }
            LineKind::IndentedField { .. } => {
                // Stray indented line at top level; consume and ignore.
                lines.next();
            }
            LineKind::Other => {
                if is_blank(&line) {
                    lines.next();
                    continue;
                }
                return Err(ParseError::new(
                    ParseErrorKind::UnexpectedContent,
                    line.line_no,
                    1,
                    line.start,
                ));
            }
        }
    }

    let doc_span = Span::new(0, input.len(), 1, 1);
    Ok(Document {
        title,
        uid,
        version,
        date,
        classification,
        prefix,
        options,
        grammar_import,
        body,
        span: doc_span,
    })
}

/// Read the body of a `[GRAMMAR]` block. Returns the `IMPORT_FROM_FILE:`
/// path if one is found.
///
/// StrictDoc `[GRAMMAR]` blocks come in two shapes: a single
/// `IMPORT_FROM_FILE:` reference to an external `.sgra` file, or an inline
/// grammar declaration that uses a YAML-like syntax (`ELEMENTS:`,
/// `FIELDS:`, leading `- ` list items, deep indentation). v0.1 treats the
/// grammar as opaque metadata — we capture the import path when present
/// and skip everything else until the next top-level block boundary.
// r[impl doc.grammar-import]
fn read_grammar_block<'a, I>(
    lines: &mut std::iter::Peekable<I>,
) -> Result<Option<String>, ParseError>
where
    I: Iterator<Item = Line<'a>>,
{
    let mut import_path = None;
    while let Some(&line) = lines.peek() {
        let kind = classify(&line);
        match kind {
            LineKind::BlockHeader { .. }
            | LineKind::DoubleBlockHeader { .. }
            | LineKind::BlockClose { .. } => break,
            LineKind::Field {
                name: "IMPORT_FROM_FILE",
                value,
            } => {
                import_path = Some(value.to_string());
                lines.next();
            }
            _ => {
                // Any other content inside [GRAMMAR] is treated as opaque
                // grammar definition: consume and continue.
                lines.next();
            }
        }
    }
    Ok(import_path)
}

/// Read a `[[SECTION]]` block (header already consumed). Returns when the
/// matching `[[/SECTION]]` is consumed.
// r[impl sect.open-close]
// r[impl sect.title]
// r[impl sect.nesting]
fn read_section_block<'a, I>(
    lines: &mut std::iter::Peekable<I>,
    header_line: Line<'a>,
) -> Result<Section, ParseError>
where
    I: Iterator<Item = Line<'a>>,
{
    let mut title: Option<String> = None;
    let mut children: Vec<DocumentChild> = Vec::new();

    while let Some(&line) = lines.peek() {
        let kind = classify(&line);
        match kind {
            LineKind::BlockClose { tag: "SECTION" } => {
                let end_offset = line.end;
                lines.next();
                let title = title.ok_or_else(|| {
                    ParseError::new(
                        ParseErrorKind::MissingSectionTitle,
                        header_line.line_no,
                        1,
                        header_line.start,
                    )
                })?;
                return Ok(Section {
                    title,
                    children,
                    span: Span::new(header_line.start, end_offset, header_line.line_no, 1),
                });
            }
            LineKind::BlockClose { tag } => {
                return Err(ParseError::new(
                    ParseErrorKind::UnknownBlock {
                        tag: format!("/{tag}"),
                    },
                    line.line_no,
                    1,
                    line.start,
                ));
            }
            LineKind::DoubleBlockHeader { tag: "SECTION" } => {
                lines.next();
                let nested = read_section_block(lines, line)?;
                children.push(DocumentChild::Section(nested));
            }
            LineKind::BlockHeader { tag: "REQUIREMENT" } => {
                lines.next();
                let req = read_requirement_block(lines, line)?;
                children.push(DocumentChild::Requirement(req));
            }
            LineKind::Field {
                name: "TITLE",
                value,
            } if title.is_none() => {
                title = Some(value.to_string());
                lines.next();
            }
            LineKind::Field { .. }
            | LineKind::EmptyField { .. }
            | LineKind::HeredocOpen { .. }
            | LineKind::IndentedField { .. } => {
                // Unrecognised section-header field; skip for forward compat.
                lines.next();
            }
            LineKind::HeredocClose => {
                return Err(ParseError::new(
                    ParseErrorKind::UnexpectedContent,
                    line.line_no,
                    1,
                    line.start,
                ));
            }
            LineKind::BlockHeader { tag: _ } => {
                lines.next();
                skip_block_body(lines, None);
            }
            LineKind::DoubleBlockHeader { tag } => {
                let close_tag = tag.to_string();
                lines.next();
                skip_block_body(lines, Some(&close_tag));
            }
            LineKind::Other => {
                if is_blank(&line) {
                    lines.next();
                    continue;
                }
                return Err(ParseError::new(
                    ParseErrorKind::UnexpectedContent,
                    line.line_no,
                    1,
                    line.start,
                ));
            }
        }
    }

    Err(ParseError::new(
        ParseErrorKind::UnclosedSection,
        header_line.line_no,
        1,
        header_line.start,
    ))
}

/// Read a `[REQUIREMENT]` block (header already consumed). The block ends
/// at the next block boundary or EOF.
// r[impl req.fields]
// r[impl req.field-order]
// r[impl req.uid-verbatim]
fn read_requirement_block<'a, I>(
    lines: &mut std::iter::Peekable<I>,
    header_line: Line<'a>,
) -> Result<Requirement, ParseError>
where
    I: Iterator<Item = Line<'a>>,
{
    let mut fields: Vec<Field> = Vec::new();
    let mut end_offset: usize = header_line.end;

    while let Some(&line) = lines.peek() {
        let kind = classify(&line);
        match kind {
            LineKind::BlockHeader { .. }
            | LineKind::DoubleBlockHeader { .. }
            | LineKind::BlockClose { .. } => break,
            LineKind::Field { name, value } => {
                let span = Span::new(line.start, line.end, line.line_no, 1);
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
                    span,
                });
                end_offset = line.end;
                lines.next();
            }
            LineKind::HeredocOpen { name } => {
                let header_span_start = line.start;
                let header_line_no = line.line_no;
                let name = name.to_string();
                lines.next();
                let (text, body_span, close_end) = read_heredoc_body(lines, line)?;
                fields.push(Field {
                    name,
                    value: FieldValue::Heredoc {
                        text,
                        span: body_span,
                    },
                    span: Span::new(header_span_start, close_end, header_line_no, 1),
                });
                end_offset = close_end;
            }
            LineKind::EmptyField { name } => {
                // KEY: with no value, possibly followed by an indented
                // sub-block (e.g. RELATIONS:). Capture the indented lines
                // as a Heredoc-style FieldValue so data is preserved.
                let header_start = line.start;
                let header_line_no = line.line_no;
                let name = name.to_string();
                lines.next();
                let (text, body_span) = read_indented_block_text(lines, line);
                let end = body_span.end.max(line.end);
                fields.push(Field {
                    name,
                    value: FieldValue::Heredoc {
                        text,
                        span: body_span,
                    },
                    span: Span::new(header_start, end, header_line_no, 1),
                });
                end_offset = end;
            }
            LineKind::IndentedField { .. } => {
                // Stray indented line; consume and ignore.
                end_offset = line.end;
                lines.next();
            }
            LineKind::HeredocClose => {
                return Err(ParseError::new(
                    ParseErrorKind::UnexpectedContent,
                    line.line_no,
                    1,
                    line.start,
                ));
            }
            LineKind::Other => {
                if is_blank(&line) {
                    lines.next();
                    continue;
                }
                return Err(ParseError::new(
                    ParseErrorKind::UnexpectedContent,
                    line.line_no,
                    1,
                    line.start,
                ));
            }
        }
    }

    Ok(Requirement {
        fields,
        span: Span::new(header_line.start, end_offset, header_line.line_no, 1),
    })
}

/// Read the body of a heredoc that has already had its opening `KEY: >>>`
/// line consumed. Returns the body text (verbatim, with leading and trailing
/// blank lines stripped), the body's span, and the end byte offset of the
/// closing `<<<` line.
// r[impl req.heredoc-verbatim]
fn read_heredoc_body<'a, I>(
    lines: &mut std::iter::Peekable<I>,
    opener: Line<'a>,
) -> Result<(String, Span, usize), ParseError>
where
    I: Iterator<Item = Line<'a>>,
{
    let body_start_offset_marker = lines.peek().map(|l| l.start).unwrap_or(opener.end);
    let body_start_line = lines
        .peek()
        .map(|l| l.line_no)
        .unwrap_or(opener.line_no + 1);

    let mut content_lines: Vec<Line<'_>> = Vec::new();
    let close_end;
    loop {
        let line = match lines.next() {
            Some(l) => l,
            None => {
                return Err(ParseError::new(
                    ParseErrorKind::UnterminatedHeredoc,
                    opener.line_no,
                    1,
                    opener.start,
                ));
            }
        };
        if matches!(classify(&line), LineKind::HeredocClose) {
            close_end = line.end;
            break;
        }
        content_lines.push(line);
    }

    // Strip leading/trailing blank lines from the collected content.
    while content_lines
        .first()
        .map(|l| l.text.chars().all(|c| c.is_whitespace()))
        .unwrap_or(false)
    {
        content_lines.remove(0);
    }
    while content_lines
        .last()
        .map(|l| l.text.chars().all(|c| c.is_whitespace()))
        .unwrap_or(false)
    {
        content_lines.pop();
    }

    let text = content_lines
        .iter()
        .map(|l| l.text)
        .collect::<Vec<_>>()
        .join("\n");

    let body_end_offset = content_lines
        .last()
        .map(|l| l.end)
        .unwrap_or(body_start_offset_marker);
    let span = Span::new(
        body_start_offset_marker,
        body_end_offset,
        body_start_line,
        1,
    );
    Ok((text, span, close_end))
}

/// Read indented `KEY: value` lines into the provided map. Stops at the
/// first non-indented or block-boundary line.
fn read_indented_subblock<'a, I>(
    lines: &mut std::iter::Peekable<I>,
    out: &mut BTreeMap<String, String>,
) where
    I: Iterator<Item = Line<'a>>,
{
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

/// Read an indented block as a single verbatim text blob (for empty fields
/// inside requirements where we don't yet have structured handling, like
/// `RELATIONS:` with its YAML-list body).
fn read_indented_block_text<'a, I>(
    lines: &mut std::iter::Peekable<I>,
    opener: Line<'a>,
) -> (String, Span)
where
    I: Iterator<Item = Line<'a>>,
{
    let mut collected: Vec<&str> = Vec::new();
    let mut start_offset = opener.end;
    let mut end_offset = opener.end;
    let mut start_line = opener.line_no + 1;
    let mut started = false;

    while let Some(&line) = lines.peek() {
        let kind = classify(&line);
        match kind {
            LineKind::IndentedField { .. } => {
                if !started {
                    start_offset = line.start;
                    start_line = line.line_no;
                    started = true;
                }
                end_offset = line.end;
                collected.push(line.text);
                lines.next();
            }
            LineKind::Other if is_blank(&line) => {
                lines.next();
            }
            LineKind::Other => {
                // YAML-list-style continuation, e.g. `- TYPE: Parent`.
                if !started {
                    start_offset = line.start;
                    start_line = line.line_no;
                    started = true;
                }
                end_offset = line.end;
                collected.push(line.text);
                lines.next();
            }
            _ => break,
        }
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
    let lead_ws = line_text.chars().take_while(|c| c.is_whitespace()).count();
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

fn skip_blank<'a, I>(lines: &mut std::iter::Peekable<I>)
where
    I: Iterator<Item = Line<'a>>,
{
    while let Some(line) = lines.peek() {
        if is_blank(line) {
            lines.next();
        } else {
            break;
        }
    }
}

/// Consume the body of an unknown block.
///
/// - If `close_tag` is `Some(t)`, consume until a matching `[[/t]]` close
///   is seen and consumed. Nested same-tag blocks bump and decrement a
///   depth counter so they're handled correctly.
/// - If `close_tag` is `None`, the block is a single-bracket open with no
///   close marker; consume until any block boundary (`[TAG]`,
///   `[[TAG]]`, or `[[/TAG]]`) is reached, leaving that boundary
///   un-consumed for the caller.
///
/// Heredoc structure inside the body is honoured (we skip from `>>>` to
/// `<<<` as one unit) so stray bracket-like sequences inside heredoc text
/// don't confuse us.
fn skip_block_body<'a, I>(lines: &mut std::iter::Peekable<I>, close_tag: Option<&str>)
where
    I: Iterator<Item = Line<'a>>,
{
    let mut depth: usize = 1;
    while let Some(&line) = lines.peek() {
        match classify(&line) {
            LineKind::HeredocOpen { .. } => {
                lines.next();
                for inner in lines.by_ref() {
                    if matches!(classify(&inner), LineKind::HeredocClose) {
                        break;
                    }
                }
            }
            LineKind::DoubleBlockHeader { tag } if close_tag == Some(tag) => {
                depth += 1;
                lines.next();
            }
            LineKind::BlockClose { tag } if close_tag == Some(tag) => {
                depth -= 1;
                lines.next();
                if depth == 0 {
                    return;
                }
            }
            LineKind::BlockHeader { .. }
            | LineKind::DoubleBlockHeader { .. }
            | LineKind::BlockClose { .. }
                if close_tag.is_none() =>
            {
                return;
            }
            _ => {
                lines.next();
            }
        }
    }
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
}
