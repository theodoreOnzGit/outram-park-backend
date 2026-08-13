#!/usr/bin/env bash
# Locate the `kovan` binary for the token-accounting hooks.
#
# Sourced by prepare-commit-msg and post-commit; sets KOVAN_BIN, or leaves it
# empty when no binary can be found.
#
# WHY THIS EXISTS
# ---------------
# Token accounting used to run `python3 docs/historian/token_usage.py`. That is
# gone (epic op-yz7b): a Python interpreter is one more thing that has to be
# installed, on PATH, and not shadowed — on Windows, `python3` routinely
# resolves to a Microsoft Store alias stub that prints an advert and exits,
# which silently turned these hooks into no-ops and let commits ship with no
# API-Usage trailer at all.
#
# The trade-off is that a binary must be BUILT or INSTALLED, where a script
# merely had to exist. Resolution order below, cheapest first.
#
# NEVER BLOCK A COMMIT
# --------------------
# Finding nothing is not an error. The caller degrades to a missing trailer,
# which is recoverable; a blocked commit is not.

# 1. An installed `kovan` on PATH (`cargo install --path crates/kovan-cli`).
# 2. A release build in this workspace (`cargo build --release -p kovan-cli`).
# 3. A debug build, as a last resort — correct, just slower to have been built.
_kovan_find() {
    if command -v kovan >/dev/null 2>&1; then
        command -v kovan
        return
    fi
    local root
    root="$(git rev-parse --show-toplevel 2>/dev/null)" || return
    for candidate in \
        "$root/target/release/kovan" "$root/target/release/kovan.exe" \
        "$root/target/debug/kovan" "$root/target/debug/kovan.exe"; do
        if [ -x "$candidate" ]; then
            printf '%s\n' "$candidate"
            return
        fi
    done
}

KOVAN_BIN="$(_kovan_find || true)"
export KOVAN_BIN
