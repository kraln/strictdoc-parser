// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Flat-iterator views over a [`Document`]'s nodes.
//!
//! This adapter walks a parsed [`Document`] and surfaces each node as a
//! [`RequirementView`] that holds enough information for downstream
//! coverage tools (notably tracey) to map a StrictDoc node onto their own
//! internal types.
//!
//! The adapter is intentionally format-agnostic on the API surface — it
//! takes no marq/tracey dependency. Callers do their own type translation.

use crate::ast::{Document, DocumentChild, Field, Node, Relation};

/// A borrowed view over a single node.
pub struct RequirementView<'a> {
    pub requirement: &'a Node,
    /// Section titles from outermost to innermost containing this node.
    /// Composite nodes are not sections and do not appear here.
    pub section_path: Vec<&'a str>,
}

impl<'a> RequirementView<'a> {
    /// The element tag, e.g. `"REQUIREMENT"`.
    pub fn node_type(&self) -> &'a str {
        &self.requirement.node_type
    }

    /// Convenience: the UID field, if present.
    pub fn uid(&self) -> Option<&'a str> {
        self.requirement.field_text("UID")
    }

    /// Convenience: the TITLE field, if present.
    pub fn title(&self) -> Option<&'a str> {
        self.requirement.field_text("TITLE")
    }

    /// Convenience: the STATEMENT field, if present.
    pub fn statement(&self) -> Option<&'a str> {
        self.requirement.field_text("STATEMENT")
    }

    /// Convenience: every field, in source order.
    pub fn fields(&self) -> &'a [Field] {
        &self.requirement.fields
    }

    /// Convenience: the node's `RELATIONS:` entries.
    pub fn relations(&self) -> &'a [Relation] {
        &self.requirement.relations
    }
}

impl Document {
    /// Walk this document in source order, producing one view per
    /// normative node: every node except `[TEXT]`, including custom-grammar
    /// elements and composite nodes, and descending into composite nodes.
    // r[impl node.flat]
    pub fn requirements_flat(&self) -> Vec<RequirementView<'_>> {
        let mut out = self.nodes_flat();
        out.retain(|v| v.requirement.is_normative());
        out
    }

    /// Walk this document in source order, producing one view per node,
    /// `[TEXT]` nodes included. A composite node precedes its children.
    pub fn nodes_flat(&self) -> Vec<RequirementView<'_>> {
        let mut out = Vec::new();
        let mut path: Vec<&str> = Vec::new();
        walk(&self.body, &mut path, &mut out);
        out
    }
}

fn walk<'a>(
    children: &'a [DocumentChild],
    section_path: &mut Vec<&'a str>,
    out: &mut Vec<RequirementView<'a>>,
) {
    for child in children {
        match child {
            DocumentChild::Section(s) => {
                section_path.push(s.title.as_str());
                walk(&s.children, section_path, out);
                section_path.pop();
            }
            DocumentChild::Node(n) => {
                out.push(RequirementView {
                    requirement: n,
                    section_path: section_path.clone(),
                });
                walk(&n.children, section_path, out);
            }
            DocumentChild::DocumentFromFile(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::parse;

    #[test]
    fn flat_walk_includes_section_path() {
        let input = "[DOCUMENT]\nTITLE: T\n\n[[SECTION]]\nTITLE: Outer\n\n[[SECTION]]\nTITLE: Inner\n\n[REQUIREMENT]\nUID: X-1\nSTATEMENT: hello\n\n[[/SECTION]]\n[[/SECTION]]\n";
        let doc = parse(input).expect("must parse");
        let reqs = doc.requirements_flat();
        assert_eq!(reqs.len(), 1);
        assert_eq!(reqs[0].uid(), Some("X-1"));
        assert_eq!(reqs[0].section_path, vec!["Outer", "Inner"]);
    }
}
