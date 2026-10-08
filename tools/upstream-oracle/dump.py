# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.

"""Record what upstream StrictDoc makes of the upstream test corpus.

Writes two JSON-lines files that tests/upstream_differential.rs compares the
crate against:

- sdoc.jsonl: for every .sdoc file in the corpus, the document header and a
  flat list of nodes (type, section path, fields, relations) as read by
  upstream's SDReader, or the error upstream reports.
- markers.jsonl: for every @relation(...) occurrence in the corpus's
  non-.sdoc files (sliced from the marker onwards), plus the hand-written
  cases in marker-cases.txt, the markers upstream's MarkerLexer finds.

Run via tools/upstream-oracle/regenerate.sh, which pins the strictdoc version
to the corpus submodule's tag.
"""

import contextlib
import io
import json
import os
import subprocess
import sys

from strictdoc.backend.sdoc.reader import SDReader
from strictdoc.backend.sdoc_source_code.comment_parser.marker_lexer import (
    MarkerLexer,
)
from strictdoc.backend.sdoc_source_code.helpers.comment_preprocessor import (
    preprocess_source_code_comment,
)


def node_title(node):
    fields = node.ordered_fields_lookup.get("TITLE")
    return fields[0].get_text_value() if fields else None


def dump_relation(ref):
    target = getattr(ref, "ref_uid", None)
    if target is None and hasattr(ref, "g_file_entry"):
        entry = ref.g_file_entry
        target = entry.g_file_path or entry.g_deprecated_file_path
    return {"type": ref.ref_type, "role": ref.role, "target": target}


def walk(elements, path, parent, out):
    for element in elements:
        node_type = getattr(element, "node_type", None)
        if type(element).__name__ == "DocumentFromFile":
            out.append(
                {"type": "DOCUMENT_FROM_FILE", "path": path, "parent": parent,
                 "file": element.file}
            )
        elif node_type == "SECTION":
            walk(element.section_contents, path + [node_title(element)], parent, out)
        elif node_type is not None:
            out.append(
                {
                    "type": node_type,
                    "path": path,
                    "parent": parent,
                    "composite": bool(element.is_composite),
                    "fields": [
                        [f.field_name, f.get_text_value(), f.is_multiline()]
                        for f in element.fields_as_parsed
                    ],
                    "relations": [dump_relation(r) for r in element.relations],
                }
            )
            if element.section_contents:
                walk(element.section_contents, path, len(out) - 1, out)
        else:
            raise AssertionError(f"unexpected element {type(element).__name__}")


def dump_sdoc(root, path):
    record = {"file": os.path.relpath(path, root)}
    try:
        with open(path, encoding="utf-8") as fh:
            content = fh.read()
        with contextlib.redirect_stdout(io.StringIO()):
            doc = SDReader.read(content, file_path=path)
        config = doc.config
        nodes = []
        walk(doc.section_contents, [], None, nodes)
        record.update(
            ok=True,
            doc={
                "title": doc.title,
                "uid": config.uid,
                "version": config.version,
                "date": config.date,
                "classification": config.classification,
                "prefix": config.requirement_prefix,
            },
            nodes=nodes,
        )
    except Exception as error:  # noqa: BLE001 — any rejection is data here
        record.update(ok=False, error=type(error).__name__)
    return record


def dump_markers(text):
    """Markers upstream finds in one comment text, or None on a parse error."""
    try:
        with contextlib.redirect_stdout(io.StringIO()):
            tree = MarkerLexer.parse(preprocess_source_code_comment(text))
    except Exception:  # noqa: BLE001
        return None
    markers = []
    for element in tree.children:
        if getattr(element, "data", None) != "relation_marker":
            continue
        marker = {"uids": [], "scope": None, "role": None}
        for child in element.children:
            value = child.children[0].value
            if child.data == "relation_node_uid":
                marker["uids"].append(value)
            elif child.data == "relation_scope":
                marker["scope"] = value
            elif child.data == "relation_role":
                marker["role"] = value
        # MarkerParser rejects duplicate UIDs after lexing.
        if len(set(marker["uids"])) != len(marker["uids"]):
            return None
        markers.append(marker)
    return markers


def marker_inputs(root, cases_path):
    inputs = []
    files = subprocess.run(
        ["git", "ls-files"], cwd=root, check=True, capture_output=True, text=True
    ).stdout.splitlines()
    for rel in sorted(files):
        if rel.endswith(".sdoc"):
            continue
        try:
            with open(os.path.join(root, rel), encoding="utf-8") as fh:
                lines = fh.read().splitlines()
        except (UnicodeDecodeError, IsADirectoryError, FileNotFoundError):
            continue
        for line in lines:
            pos = line.find("@relation")
            while pos >= 0:
                inputs.append(line[pos:])
                pos = line.find("@relation", pos + 1)
    with open(cases_path, encoding="utf-8") as fh:
        for line in fh:
            line = line.rstrip("\n")
            if line and not line.startswith(";"):
                # Cases are written on one line; `\n` stands for a line break.
                inputs.append(line.replace("\\n", "\n"))
    return list(dict.fromkeys(inputs))


def main():
    corpus_root, cases_path, out_dir = sys.argv[1:4]
    with open(os.path.join(out_dir, "sdoc.jsonl"), "w", encoding="utf-8") as out:
        for dirpath, dirnames, filenames in os.walk(corpus_root):
            dirnames.sort()
            for name in sorted(filenames):
                if name.endswith(".sdoc"):
                    record = dump_sdoc(corpus_root, os.path.join(dirpath, name))
                    out.write(json.dumps(record, ensure_ascii=False) + "\n")
    with open(os.path.join(out_dir, "markers.jsonl"), "w", encoding="utf-8") as out:
        for text in marker_inputs(corpus_root, cases_path):
            markers = dump_markers(text)
            record = {"input": text}
            if markers is None:
                record["error"] = True
            else:
                record["markers"] = markers
            out.write(json.dumps(record, ensure_ascii=False) + "\n")


if __name__ == "__main__":
    main()
