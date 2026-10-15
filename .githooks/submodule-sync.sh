#!/usr/bin/env bash
# Bring already-initialised submodules to the commits the superproject now
# records. Called by post-merge (git pull with merge), post-rewrite (git pull
# --rebase) and post-checkout (branch switch). Never blocks, never initialises
# a submodule nobody asked for (the literature submodule is large), and skips
# any submodule with uncommitted work so local edits are never touched.
set -u
ROOT="$(git rev-parse --show-toplevel 2>/dev/null)" || exit 0
cd "$ROOT" || exit 0
[ -f .gitmodules ] || exit 0
git submodule status 2>/dev/null | while read -r line; do
  case "$line" in
    -*) continue ;;              # not initialised: leave it alone
    +*) ;;                       # checked out at a different commit: sync it
    *) continue ;;               # already at the recorded commit
  esac
  path="$(echo "$line" | awk '{print $2}')"
  if [ -n "$(git -C "$path" status --porcelain 2>/dev/null)" ]; then
    echo "submodule-sync: $path has uncommitted changes; not updated." >&2
    continue
  fi
  git submodule update --quiet -- "$path" \
    && echo "submodule-sync: $path -> $(git -C "$path" rev-parse --short HEAD)" \
    || echo "submodule-sync: updating $path failed; run 'git submodule update $path'." >&2
done
exit 0
