#!/usr/bin/env bash
#
# Lockstep release, step 2 of 3: check everything that would make a publish
# fail half-way, WITHOUT publishing anything (gh:#971, #972).
#
#   scripts/release/preflight.sh
#
# Refuses (exit 1) unless:
#   1. the crates, manifests and lockfile have no uncommitted changes;
#   2. HEAD carries a `release-*` tag;
#   3. every member crate ships a LICENSE (the workspace root's does not travel);
#   4. every internal normal/build dependency carries a version (crates.io
#      rejects a bare path; dev-dependencies are stripped and may stay bare);
#   5. every crate name is free on crates.io or owned by theodoreOnzGit (this
#      would have caught `dover`, owned by another project);
#   6. no crate's new version is already on crates.io;
#   7. `cargo package --workspace --no-verify` packs every crate, each .crate
#      under crates.io's 10 MiB limit (packing one crate at a time fails until
#      its dependencies' new versions are live, so it is done all at once);
#   8. `cargo publish --workspace --dry-run` passes. It builds every package
#      from its tarball, so it takes a while.

set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
. "$here/common.sh"
cd "$(repo_root)"

fail=0
bad() { echo "  FAIL: $*"; fail=1; }

echo "1. clean tree"
[ -z "$(git status --porcelain -- crates Cargo.toml Cargo.lock)" ] || bad "uncommitted changes under crates/, Cargo.toml or Cargo.lock"

echo "2. release tag"
tag=$(git tag --points-at HEAD | grep '^release-' | head -1 || true)
[ -n "$tag" ] || bad "HEAD has no release-* tag"

echo "3. LICENSE in every crate"
while IFS=$'\t' read -r name ver dir; do
    [ -f "$dir/LICENSE" ] || bad "$name has no LICENSE"
done < <(members)

echo "4. every internal dependency versioned"
unversioned=$(cargo metadata --no-deps --format-version 1 | jq -r '
    [.packages[].name] as $ws
    | .packages[] | .name as $p
    | .dependencies[]
    | select(.path != null and .req == "*" and .kind != "dev" and (.name as $n | $ws | index($n)))
    | "\($p) -> \(.name)"')
[ -z "$unversioned" ] || bad "unversioned path dependencies:"$'\n'"$unversioned"

echo "5/6. names and versions on crates.io"
while IFS=$'\t' read -r name ver dir; do
    case $(api_status "$name") in
        404) ;;
        200)
            owners "$name" | grep -qx "$OWNER" || bad "$name on crates.io is not owned by $OWNER"
            version_is_live "$name" "$ver" && bad "$name $ver is already on crates.io"
            ;;
        *) bad "crates.io lookup of $name failed" ;;
    esac
done < <(members)

echo "7. packaging and size"
if cargo package --workspace --no-verify >/dev/null 2>&1; then
    while IFS=$'\t' read -r name ver dir; do
        f="target/package/$name-$ver.crate"
        [ -f "$f" ] || { bad "$f not produced"; continue; }
        size=$(stat -c %s "$f")
        [ "$size" -lt 10485760 ] || bad "$name $ver packs to $size bytes, over 10 MiB"
    done < <(members)
else
    bad "cargo package --workspace --no-verify failed"
fi

[ "$fail" = 0 ] || die "preflight failed before the dry run; fix the above"

echo "8. cargo publish --workspace --dry-run"
cargo publish --workspace --dry-run || die "dry run failed"

echo "Preflight passed for $tag."
