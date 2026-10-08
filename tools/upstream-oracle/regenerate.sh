#!/bin/sh
# Regenerate tests/fixtures/upstream-oracle/{sdoc,markers}.jsonl from the
# strictdoc release matching the corpus submodule's tag. Requires `uv`.
set -eu
repo=$(cd "$(dirname "$0")/../.." && pwd)
corpus="$repo/tests/fixtures/upstream/strictdoc"
version=$(git -C "$corpus" describe --tags --exact-match)
out="$repo/tests/fixtures/upstream-oracle"
echo "strictdoc $version" > "$out/VERSION"
uv run --no-project --isolated --with "strictdoc==$version" \
    python -I "$repo/tools/upstream-oracle/dump.py" \
    "$corpus" "$out/marker-cases.txt" "$out"
