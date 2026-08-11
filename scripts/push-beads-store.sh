#!/usr/bin/env bash
#
# Push the kopi-beans store ref to origin, and nothing else.
#
# WHY THIS EXISTS
# ---------------
# kopi-beans (`bn`) keeps its canonical state in git refs, not in working-tree
# files: `refs/heads/beads/store` holds state.jsonl / deps.jsonl /
# tombstones.jsonl / meta.json. Historically `bn` could not publish that ref to
# a non-local remote (kopitiam#19), so every bead change had to be pushed by
# hand or it existed only on one machine. This script automates that push so
# beads filed in a session are never stranded locally.
#
# SCOPE — read before editing
# ---------------------------
# This script pushes EXACTLY ONE refspec:
#
#     refs/heads/beads/store:refs/heads/beads/store
#
# It must never be extended to push branches, tags, or anything else. The
# workspace's never-auto-push rule still governs all ordinary code; the narrow
# carve-out authorised by the maintainer covers the beads store ref ONLY, and
# explicitly never `main`. If you find yourself adding a second refspec here,
# stop — that needs the maintainer to ask for it directly.
#
# BEHAVIOUR
# ---------
# - No local store ref            -> exit 0 quietly (nothing to publish).
# - Local ref matches remote      -> exit 0 quietly (no redundant network call).
# - Local ref differs from remote -> push it, report one line.
# - Network/remote failure        -> warn on stderr, exit 0.
#
# The last point is deliberate: this runs from a Claude Code Stop hook, and a
# transient network failure must not fail the session or block the user. A
# missed push is recoverable on the next run; a broken session is not.

set -uo pipefail

REF="refs/heads/beads/store"
REMOTE="${BEADS_REMOTE:-origin}"

# Only operate inside a git work tree.
git rev-parse --is-inside-work-tree >/dev/null 2>&1 || exit 0

# Nothing to publish if the store ref does not exist locally.
local_sha="$(git rev-parse --verify --quiet "$REF" 2>/dev/null)" || exit 0
[ -n "$local_sha" ] || exit 0

# Compare against the remote before spending a push.
remote_sha="$(git ls-remote "$REMOTE" "$REF" 2>/dev/null | awk '{print $1}')"

if [ "$local_sha" = "$remote_sha" ]; then
  exit 0
fi

if git push "$REMOTE" "$REF:$REF" >/dev/null 2>&1; then
  echo "beads store ref pushed to $REMOTE (${local_sha:0:12})"
else
  echo "warning: could not push beads store ref to $REMOTE; beads remain local only" >&2
fi

exit 0
