#!/usr/bin/env bash
#
# csl-reference.sh — regenerate the citeproc-js reference that
# kovan-literature's CSL citations (`csl`, GitHub #789) and the citeproc-js
# port (GitHub #790, #791) are tested against.
#
# 1. Writes the site .bib as CSL-JSON, exactly as the port produces it, to
#    crates/kovan-literature/tests/data/csl/items.json (kovan-cli).
# 2. Fetches (once, git-ignored vendor/) citeproc-js and the CSL test suite at
#    their pinned commits, and checks that citeproc-js's src/ builds the same
#    engine as npm citeproc 2.4.63 (scripts/csl-verify-citeproc-build.cjs).
# 3. Runs that CSL-JSON, the five site styles (apa, chicago-author-date, ieee,
#    nature, vancouver = nlm-citation-sequence) and their locales through
#    citeproc-js (Zotero's CSL engine): tests/data/csl/reference_<style>.json
#    (scripts/csl-reference.cjs).
# 4. Runs the CSL test suite's processor fixtures through citeproc-js exactly
#    as its own test runner does: tests/data/csl/test_suite_reference.json
#    (scripts/csl-testsuite-reference.cjs).
#    The reference uses citeproc-js's pinned 2019 locales; tests/citeproc_test_suite.rs
#    runs the port on the CURRENT locales (vendor/csl-locales, D13).
#
# citeproc-js is installed with npm into target/csl-reference/ (git-ignored),
# pinned to CITEPROC_VERSION. Needs node (12 is enough), npm and git. Review
# the diff before committing: a changed reference means the .bib, a style, the
# locales or the engine changed.

set -euo pipefail

CITEPROC_VERSION="${CITEPROC_VERSION:-2.4.63}"
CITEPROC_JS_COMMIT=73bc1b44bc7d54d0bfec4e070fd27f5efe024ff9
CSL_TEST_SUITE_COMMIT=6eefc5b07c6969ab8999e48542acbcc131cba864
# Current CSL locale data (2026-09-10; byte-identical to the site's
# crates/kovan-literature/data/csl/locales-en-*.xml). CC BY-SA 3.0. The test
# suite fixtures are verified against it (DEVIATIONS.md D13).
CSL_LOCALES_COMMIT=a89adece41013402236e2c9020972d7e931fbab8
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

# vendor/ is git-ignored (workspace vendor rule).
fetch() { # <url> <dir> <commit>
	if [[ ! -d "$2/.git" ]]; then
		git clone --quiet "$1" "$2"
	fi
	git -C "$2" checkout --quiet "$3"
}
fetch https://github.com/juris-m/citeproc-js vendor/citeproc-js "$CITEPROC_JS_COMMIT"
fetch https://github.com/citation-style-language/test-suite vendor/csl-test-suite "$CSL_TEST_SUITE_COMMIT"
fetch https://github.com/citation-style-language/locales vendor/csl-locales "$CSL_LOCALES_COMMIT"
# The locales citeproc-js 2.4.63 pins: the directory its test runner reads.
git -C vendor/citeproc-js submodule update --init --quiet locale

export CITEPROC_MODULE="$prefix/node_modules/citeproc/citeproc_commonjs.js"
node scripts/csl-verify-citeproc-build.cjs
node scripts/csl-reference.cjs
node scripts/csl-testsuite-reference.cjs
echo "csl-reference: wrote crates/kovan-literature/tests/data/csl/reference_*.json and test_suite_reference.json"
