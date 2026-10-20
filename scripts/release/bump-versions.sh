#!/usr/bin/env bash
#
# Lockstep release, step 1 of 3: bump every member crate's version
# (gh:#971, #972; maintainer scheme, 2026-10-20).
#
#   scripts/release/bump-versions.sh            # rewrite the manifests
#   scripts/release/bump-versions.sh --dry-run  # print the table only
#
# The scheme:
#   - a crate not yet on crates.io gets 0.0.1;
#   - every other crate gets its patch version +1;
#   - release/overrides.toml sets a crate's version outright
#     (`crate-name = "x.y.z"`). An override is applied only when it is above
#     the crate's current version, so last release's overrides are inert and
#     the file need not be emptied by hand.
#
# It rewrites each crate's own `version` and every internal pin in the root
# and member manifests, both `name = { ..., version = "..." }` and
# `alias = { package = "name", ..., version = "..." }`. Bumping versions needs
# the maintainer's explicit request (root CLAUDE.md); this script is the
# mechanism, not the permission.
#
# Then: run the full release test suite, commit, tag, and run preflight.sh.

set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
. "$here/common.sh"
cd "$(repo_root)"

dry=0
[ "${1:-}" = "--dry-run" ] && dry=1

overrides=release/overrides.toml
override_for() {
    [ -f "$overrides" ] || return 0
    sed -nE "s/^\"?$1\"?[[:space:]]*=[[:space:]]*\"([^\"]+)\".*/\1/p" "$overrides" | head -1
}

plan=$(mktemp)
trap 'rm -f "$plan"' EXIT

while IFS=$'\t' read -r name old dir; do
    ovr=$(override_for "$name")
    if [ -n "$ovr" ] && version_gt "$ovr" "$old"; then
        new=$ovr; why=override
    else
        case $(api_status "$name") in
            404) new=0.0.1; why="new crate" ;;
            200) IFS=. read -r a b c <<<"$old"; new="$a.$b.$((c + 1))"; why="patch +1" ;;
            *) die "crates.io lookup of $name failed" ;;
        esac
    fi
    printf '%s\t%s\t%s\t%s\t%s\n' "$name" "$old" "$new" "$dir" "$why" >>"$plan"
done < <(members)

printf '%-62s %-8s    %-8s %s\n' crate old new reason
while IFS=$'\t' read -r name old new dir why; do
    printf '%-62s %-8s -> %-8s %s\n' "$name" "$old" "$new" "$why"
done <"$plan"

[ "$dry" = 1 ] && exit 0

manifests=(Cargo.toml crates/*/Cargo.toml)
while IFS=$'\t' read -r name old new dir why; do
    # The crate's own version: the first top-level `version = ` line.
    sed -i -E "0,/^version[[:space:]]*=[[:space:]]*\"[^\"]+\"/s//version = \"$new\"/" "$dir/Cargo.toml"
    # Every pin of it, by key or by `package = ` alias.
    sed -i -E \
        -e "s/^($name[[:space:]]*=[[:space:]]*\{[^}]*\bversion[[:space:]]*=[[:space:]]*\")[^\"]+\"/\1$new\"/" \
        -e "s/(package[[:space:]]*=[[:space:]]*\"$name\"[^}]*\bversion[[:space:]]*=[[:space:]]*\")[^\"]+\"/\1$new\"/" \
        "${manifests[@]}"
done <"$plan"

cargo metadata --no-deps --format-version 1 >/dev/null || die "manifests no longer resolve"
echo "Manifests rewritten. Review with git diff, then run the full release test suite."
