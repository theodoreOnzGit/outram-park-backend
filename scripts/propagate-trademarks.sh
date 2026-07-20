#!/usr/bin/env bash
# ============================================================================
# propagate-trademarks.sh
#
# Copies TRADEMARKS.md from the monorepo root into every crate's directory,
# so each published crate carries its own copy for downstream consumers,
# crates.io display, and audit purposes.
#
# Idempotent: safe to re-run whenever TRADEMARKS.md is updated.
# Run from the monorepo root (or anywhere — the script locates itself).
# ============================================================================
set -euo pipefail

# Resolve monorepo root as the parent of this script's directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
SOURCE_FILE="${ROOT_DIR}/TRADEMARKS.md"

# --- Sanity checks ---
if [[ ! -f "${SOURCE_FILE}" ]]; then
    echo "error: ${SOURCE_FILE} not found." >&2
    echo "       Run this script from the Outram Park monorepo." >&2
    exit 1
fi

if [[ ! -f "${ROOT_DIR}/Cargo.toml" ]]; then
    echo "warning: no root Cargo.toml at ${ROOT_DIR}" >&2
    echo "         Proceeding anyway, but verify this is the right directory." >&2
fi

echo "Source:      ${SOURCE_FILE}"
echo "Monorepo:    ${ROOT_DIR}"
echo

# --- Find every crate directory ---
# A "crate" here = a directory containing a Cargo.toml, excluding:
#   - the monorepo root itself
#   - target/ build artefacts
#   - vendored dependencies under vendor/
#   - hidden directories (.git etc.)

COPIED=0
SKIPPED=0
UNCHANGED=0

while IFS= read -r -d '' cargo_toml; do
    crate_dir="$(dirname "${cargo_toml}")"

    # Skip the workspace root
    if [[ "${crate_dir}" == "${ROOT_DIR}" ]]; then
        continue
    fi

    dest="${crate_dir}/TRADEMARKS.md"

    # Skip if destination already identical (idempotent, quiet)
    if [[ -f "${dest}" ]] && cmp -s "${SOURCE_FILE}" "${dest}"; then
        printf "  unchanged: %s\n" "${crate_dir#${ROOT_DIR}/}"
        UNCHANGED=$((UNCHANGED + 1))
        continue
    fi

    cp -f "${SOURCE_FILE}" "${dest}"
    printf "  copied:    %s\n" "${crate_dir#${ROOT_DIR}/}"
    COPIED=$((COPIED + 1))

done < <(find "${ROOT_DIR}" \
    -type d \( -name target -o -name vendor -o -name .git \) -prune -o \
    -type f -name Cargo.toml -print0)

echo
echo "Summary:"
echo "  copied:    ${COPIED}"
echo "  unchanged: ${UNCHANGED}"
echo "  skipped:   ${SKIPPED}"
echo
echo "Done."
