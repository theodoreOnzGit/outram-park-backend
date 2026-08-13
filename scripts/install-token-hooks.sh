#!/usr/bin/env bash
# Install the OUTRAM PARK token-accounting git hooks in this clone.
#
# The hooks live in the version-controlled `.githooks/` dir; this script points
# git at them (`core.hooksPath`) and initialises the per-commit token baseline
# so the first accounted commit shows a sensible delta (not the whole
# session-to-date). Run once per fresh clone. Idempotent.
#
#   ./scripts/install-token-hooks.sh
#
# NOTE (epic op-yz7b): accounting is now the `kovan` binary (crates/kovan-metrics),
# not `python3 docs/historian/token_usage.py`. A binary has to exist before the
# hooks can do anything, so this script builds it when it is missing.
set -euo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

# core.hooksPath is a LOCAL git config (never committed), so every clone must run
# this. If another hooks mechanism is already configured, this overrides it for
# this clone — reconcile by copying those hooks into .githooks/ if needed.
git config core.hooksPath .githooks
chmod +x .githooks/* 2>/dev/null || true

# shellcheck source=/dev/null
. "$ROOT/.githooks/kovan-bin.sh" 2>/dev/null || true
if [ -z "${KOVAN_BIN:-}" ]; then
    echo "No 'kovan' binary found — building it (cargo build --release -p kovan-cli)…"
    cargo build --release -p kovan-cli
    # shellcheck source=/dev/null
    . "$ROOT/.githooks/kovan-bin.sh"
fi

if [ -z "${KOVAN_BIN:-}" ]; then
    echo "WARNING: still no 'kovan' binary. The hooks are installed but will be" >&2
    echo "         a no-op until one exists — commits will carry NO API-Usage" >&2
    echo "         trailer. Build it with 'cargo build --release -p kovan-cli'," >&2
    echo "         or install it with 'cargo install --path crates/kovan-cli'." >&2
    exit 0
fi

"$KOVAN_BIN" tokens init
echo "Installed: core.hooksPath -> .githooks (prepare-commit-msg + post-commit)."
echo "Accounting binary: $KOVAN_BIN"
echo "Every commit from now on will carry an API-Usage trailer and refresh docs/token-usage.md."
