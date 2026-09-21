# Why kopi-beans (`bn`) was deprecated as this workspace's issue tracker

**Maintainer decision, 2026-09-21.** `bn` / kopi-beans is **no longer the
mandated issue tracker** for OUTRAM PARK. GitHub issues
(`gh issue …`, on `theodoreOnzGit/outram-park-backend`) replace it. The rule
now lives in the root `CLAUDE.md` under "Issue tracking & roadmap"; this file
records **why**.

## The reason

**Persistent problems with the beads store made the tracker cumbersome to
operate.** kopi-beans keeps its canonical state in git refs
(`refs/heads/beads/store`, plus `refs/beads/backup/*`), and that store — and
the daemon that publishes it — was the recurring source of friction, session
after session. The tracker stopped being infrastructure that got out of the
way and became something each session had to nurse.

These are upstream defects in a tool this workspace deliberately dogfoods, not
defects in the workspace's own use of it. That is exactly what dogfooding is
for, and filing them was the right outcome each time. But the accumulated cost
of working around store problems, while the store *is* the tracker, is what
tipped the decision: a distributed tracker whose distribution mechanism needs
attention is worse than a hosted tracker that does not.

## The recorded evidence

Every item below is a real file in this repository, filed at the time with the
command run and the observed output. Five of the eight kopi-beans issues ever
filed here concern the store or the daemon that syncs it.

**Still open** (`docs/kopitiam-issues/`):

| Issue | Upstream | What happens |
|---|---|---|
| [`kopi-beans-daemon-sync-fails-slotmap-too-small.md`](kopitiam-issues/kopi-beans-daemon-sync-fails-slotmap-too-small.md) | [kopitiam#27](https://github.com/theodoreOnzGit/kopitiam/issues/27) | Daemon sync fails with a `gix` "slotmap turned out to be too small" fetch error, and `bn sync` then **hangs**. Observed on a store at `format_version 2` with ~833 issues and 74 git refs, 64 of them `refs/beads/backup/*`. |
| [`kopi-beans-daemon-burns-37-percent-cpu-continuously.md`](kopitiam-issues/kopi-beans-daemon-burns-37-percent-cpu-continuously.md) | [kopitiam#26](https://github.com/theodoreOnzGit/kopitiam/issues/26) | `bn daemon run` holds ~37 % of a CPU core continuously. Probable root cause (the uncapped push-retry loop) was fixed in 0.1.6, but the Linux `ps` reproduction has **not** been re-run there, so by this workspace's own "resolved means verified, not announced" rule it stays open. |

**Resolved** (`docs/kopitiam-issues/resolved/`) — each carries its closing
evidence:

| Issue | What happened |
|---|---|
| [`kopi-beans-store-format-version-1.md`](kopitiam-issues/resolved/kopi-beans-store-format-version-1.md) | The store was written at `format_version 1` and had to be migrated to 2 before kopi-beans could read it at all. |
| [`kopi-beans-cannot-push-store-ref.md`](kopitiam-issues/resolved/kopi-beans-cannot-push-store-ref.md) | The daemon could not publish `refs/heads/beads/store` at all. This is why `scripts/push-beads-store.sh` and the `Stop` hook in `.claude/settings.json` exist — a workaround that outlived the defect it was written for. |
| [`kopi-beans-daemon-retries-failing-push-forever.md`](kopitiam-issues/resolved/kopi-beans-daemon-retries-failing-push-forever.md) | The daemon retried a failing `git push` roughly every 2.5 s indefinitely with no backoff cap (`consecutive_failures: 30`, `last_sync: never`). Fixed in 0.1.6. |

The two remaining resolved files (`kopi-beans-help-text-branded-as-bd.md`,
`kopi-beans-installs-stray-tailnet-binary.md`) are unrelated to the store and
are listed only for completeness.

A secondary factor: version churn. kopi-beans moved
`0.1.3 → 0.1.4 → 0.1.6 → 0.1.7` in a single day at one point, which is healthy
for a young tool and awkward for something every session depends on.

## What this decision does NOT change

- **kopi-beans is not condemned, and upstream work on it is not affected.**
  This is a decision about what *this* workspace depends on day to day, taken
  while the store issues are outstanding. It is reversible.
- **KOPITIAM and KOVAN dogfooding are untouched.** The `CLAUDE.md` rules for
  `kopitiam` (token-frugal reading, symbol queries, rename/code-actions) and
  for `kovan` (literature, digitiser, API docs, metrics) all still bind, as
  does the hard boundary that this workspace **consumes** kopitiam/kopi-beans
  binaries and never modifies their source from here.
- **The upstream issue queue stays live.** `docs/kopitiam-issues/` remains the
  fallback channel for kopitiam/kopi-beans defects when `gh` is unavailable,
  and resolved issues still move to `resolved/` with their closing evidence.
- **The existing beads store is not deleted.** `refs/heads/beads/store`, the
  `refs/beads/backup/*` refs, and the pre-migration snapshot
  `refs/beads/premigration-v1-20260807` all stay where they are. Several
  hundred `op-*` identifiers are cited throughout `CLAUDE.md`, the crate docs
  and the V&V write-ups; keeping the store is what keeps those citations
  resolvable.
- **`op-*` ids are historical references from now on.** Do not mint new ones,
  and do not expect one to exist in GitHub issues.
- **`.claude/settings.json` was left untouched** by this change. It still
  carries the `SessionStart` `bn prime --mcp` hook and the `Stop`
  `push-beads-store.sh` hook. Removing them is a separate maintainer decision;
  both are harmless no-ops if `bn` is uninstalled or the ref is absent.

## Migrating the open beads

**Not done, and deliberately.** The open beads have not been bulk-imported
into GitHub issues — a few hundred auto-filed issues would bury the ones that
matter. The intended path is to open GitHub issues for work as it is actually
picked up, citing the old `op-*` id in the body where one exists. If a bulk
export is wanted later, `bn list --json` against the preserved store is the
source.
