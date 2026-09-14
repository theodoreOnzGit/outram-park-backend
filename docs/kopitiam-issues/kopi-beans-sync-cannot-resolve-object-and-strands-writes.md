# kopi-beans: `bn sync` fails with "cannot resolve an object a reference points to", and the write is silently stranded locally

**Tool:** `kopi-beans` (binary `bn`)
**Version:** `bn 0.1.9` (`cargo install --list` → `kopi-beans v0.1.9`), 2026-09-14
**Environment:** Linux 6.18.44, x86_64, remote-execution container.
Repository `theodoreOnzGit/outram-park-backend`, checked out **as a git
submodule**, so its git dir is
`<parent>/.git/modules/outram-park-backend`, not `<repo>/.git`.
Store `refs/heads/beads/store`. 25 refs total, `packed-refs` present, no
`refs/beads/backup/*` currently loose. `git count-objects -v`: 24 397 loose
objects, 148 MB, `garbage: 0`.

> **Related but not the same as**
> [`kopi-beans-daemon-sync-fails-slotmap-too-small.md`](./kopi-beans-daemon-sync-fails-slotmap-too-small.md)
> (kopitiam#27, `bn 0.1.3`). That one is a `gix` *fetch* failure with a
> "slotmap turned out to be too small" message and `bn sync` then **hangs**.
> This one is on 0.1.9, carries a different error string, and `bn sync`
> **gives up cleanly** rather than hanging. Same consequence, though: the
> write does not reach the git ref.

## What I ran

```
$ bn update op-j57z --notes "PROGRESS 2026-09-14 …"
✓ Updated issue: op-j57z

$ bn sync
 ERROR error: sync wait gave up after 6183ms for github.com/theodoreOnzGit/outram-park-backend:
 git operation failed: An error occurred when trying to resolve an object a reference points to

$ git push origin refs/heads/beads/store:refs/heads/beads/store
Everything up-to-date
```

## Observed

The update **is** applied to the local store and reads back immediately:

```
$ bn show op-j57z
Status: In Progress
Assignee: root@vm
  PROGRESS 2026-09-14 (097877617, baf9408b4): trait lifted into petir::mathf …
  [root@vm at 2026-09-14 09:42]
```

…but **the git ref never moves**:

```
$ git rev-parse refs/heads/beads/store
9389677b1a46d29da1a8e8a8f839507f2bf8dbbb    # before the update
9389677b1a46d29da1a8e8a8f839507f2bf8dbbb    # after the update and after bn sync
```

so the manual fallback push reports `Everything up-to-date` — correctly, since
there is nothing new on the ref — and the bead is **stranded on one machine**.
That is precisely the failure mode the workspace's Stop-hook carve-out exists
to prevent, and it is invisible unless you check `git rev-parse` by hand:
`bn update` exits 0, `bn show` confirms the note, and the push says
"up-to-date". Every individual signal looks like success.

## Expected

Either `bn sync` flushes the pending write onto `refs/heads/beads/store` so the
ref advances and the push publishes it, **or** — if it cannot — `bn update`
itself reports that the write is local-only. A clean-exit failure whose only
symptom is a ref that did not move is the worst of both.

## Notes toward a cause (hypotheses, not findings)

Offered as leads for whoever picks this up; none of these is verified.

1. **Submodule git dir.** This checkout's git directory is
   `<parent>/.git/modules/outram-park-backend`, reached via a `.git` *file*
   rather than a directory. If the daemon resolves the git dir by assuming
   `<repo>/.git` is a directory, it could be operating on a different (or
   partially-resolved) object store than the one the ref lives in. That would
   fit "cannot resolve an object a reference points to" exactly.
2. **Loose-object pressure.** `git` has been printing
   `warning: There are too many unreachable loose objects; run 'git prune'`
   on nearly every invocation this session, and
   `.git/modules/outram-park-backend/gc.log` contains that same line — so
   automatic gc is disabled until the log is removed. 24 397 loose objects.
   Whether this is a cause or merely a co-symptom is untested; `git prune`
   was **not** run here, because another agent session is writing to the same
   repository concurrently and pruning under that is not obviously safe.
3. **Concurrency.** Another machine's daemon advanced `refs/heads/beads/store`
   during this session (local went from 0-ahead/0-behind to 0-ahead/1-behind).
   The local ref was fast-forwarded with `git update-ref` before the failing
   `bn update`, so the write above was made against an up-to-date base — but a
   race in how the daemon reconciles a remotely-advanced ref is worth ruling
   out.

## Impact and workaround

**Impact:** medium-high for a distributed tracker. Issue *creation* did reach
the remote earlier in the same session (two new beads are on
`refs/heads/beads/store` and readable from it), so this is not total loss —
but a later `--notes` update to one of those same beads did not, with no error
at the point of writing.

**Workaround used:** none that publishes. The progress note was additionally
written into the commit message of the work it describes, so the information
is in the repository even though the bead carrying it is not. Recorded in the
session hand-off.

## Upstreaming

Not yet filed on `theodoreOnzGit/kopitiam` — that repository is outside this
session's GitHub scope. **This file is the live queue entry; upstream it and
add the issue link at the top, as
`kopi-beans-daemon-sync-fails-slotmap-too-small.md` does.**
