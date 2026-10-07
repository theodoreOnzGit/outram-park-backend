#!/usr/bin/env bash
#
# csl-reference.sh — regenerate the citeproc-js reference that
# kovan-literature's CSL citations (`csl`, GitHub #789) are tested against in
# tests/csl_vs_citeproc_js.rs.
#
# 1. Writes the site .bib as CSL-JSON, exactly as the port produces it, to
#    crates/kovan-literature/tests/data/csl/items.json (kovan-cli).
# 2. Runs that CSL-JSON, apa.csl and the en-US locale through citeproc-js
#    (Zotero's CSL engine) and writes tests/data/csl/reference_apa.json.
#
# citeproc-js is installed with npm into target/csl-reference/ (git-ignored),
# pinned to CITEPROC_VERSION. Needs node and npm. Review the diff before
# committing: a changed reference means the .bib, the style or the engine
# changed.

set -euo pipefail

CITEPROC_VERSION="${CITEPROC_VERSION:-2.4.63}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

cargo run --release -q -p knowledge-oriented-vv-analysis-for-nuclear-sciences-kovan --bin kovan-cli -- \
	references --bib docs/site/references.bib \
	--csl-json-out crates/kovan-literature/tests/data/csl/items.json

prefix="$root/target/csl-reference"
if [[ ! -f "$prefix/node_modules/citeproc/package.json" ]] ||
	! grep -q "\"version\": \"$CITEPROC_VERSION\"" "$prefix/node_modules/citeproc/package.json"; then
	npm install --silent --no-save --prefix "$prefix" "citeproc@$CITEPROC_VERSION"
fi

CITEPROC_MODULE="$prefix/node_modules/citeproc/citeproc_commonjs.js" node scripts/csl-reference.cjs
echo "csl-reference: wrote crates/kovan-literature/tests/data/csl/reference_apa.json"
