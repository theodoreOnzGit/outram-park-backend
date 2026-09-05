#!/usr/bin/env bash
# Locate the `kovan-cli` binary for the token-accounting hooks.
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
# WHY `kovan-cli`, NOT `kovan` (fixed 2026-08-22)
# ------------------------------------------------
# Before GitHub issue #30's 3-binary restructure (2026-08-21), `kovan` was
# this crate's one CLI binary and carried `tokens`/`historian`/etc. directly.
# That name is now the GUI (`kovan` = eframe window; `kovan-cli` = the
# agent/script-facing CLI these subcommands actually live on — see
# crates/kovan/README.md). This script silently kept resolving to the old
# name and finding the *GUI* binary at target/release/kovan (still built by a
# plain `cargo build --release -p kovan`, since `gui` defaults on) — so
# `prepare-commit-msg` launched a stray GUI window and hung every commit
# instead of no-op'ing or stamping a trailer. Confirmed by hand: `kovan
# tokens trailer` / `kovan --help` both hang (a GUI process appears in `ps`);
# `kovan-cli tokens trailer --help` returns immediately. Filed as a workspace
# bug fix, not a kopitiam issue — this script belongs to this repo.
#
# NEVER BLOCK A COMMIT
# --------------------
# Finding nothing is not an error. The caller degrades to a missing trailer,
# which is recoverable; a blocked commit is not.

# 1. An installed `kovan-cli` on PATH (`cargo install --path crates/kovan --bin kovan-cli`).
# 2. A release build in this workspace (`cargo build --release -p kovan --bin kovan-cli`).
# 3. A debug build, as a last resort — correct, just slower to have been built.
_kovan_find() {
    if command -v kovan-cli >/dev/null 2>&1; then
        command -v kovan-cli
        return
    fi
    local root
    root="$(git rev-parse --show-toplevel 2>/dev/null)" || return
    for candidate in \
        "$root/target/release/kovan-cli" "$root/target/release/kovan-cli.exe" \
        "$root/target/debug/kovan-cli" "$root/target/debug/kovan-cli.exe"; do
        if [ -x "$candidate" ]; then
            printf '%s\n' "$candidate"
            return
        fi
    done
}

KOVAN_BIN="$(_kovan_find || true)"
export KOVAN_BIN
