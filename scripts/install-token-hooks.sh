#!/usr/bin/env bash
# Install the OUTRAM PARK token-accounting git hooks in this clone.
#
# The hooks live in the version-controlled `.githooks/` dir; this script points
# git at them (`core.hooksPath`) and initialises the per-commit token baseline
# so the first accounted commit shows a sensible delta (not the whole
# session-to-date). Run once per fresh clone. Idempotent.
#
#   ./scripts/install-token-hooks.sh
set -euo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

# core.hooksPath is a LOCAL git config (never committed), so every clone must run
# this. If another hooks mechanism is already configured, this overrides it for
# this clone — reconcile by copying those hooks into .githooks/ if needed.
git config core.hooksPath .githooks
chmod +x .githooks/* scripts/token_accounting.py 2>/dev/null || true

python3 scripts/token_accounting.py init
echo "Installed: core.hooksPath -> .githooks (prepare-commit-msg + post-commit)."
echo "Every commit from now on will carry an API-Usage trailer and refresh docs/token-usage.md."
