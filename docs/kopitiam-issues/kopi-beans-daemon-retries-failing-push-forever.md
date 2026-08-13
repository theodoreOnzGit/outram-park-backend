# kopi-beans: the daemon retries a failing sync push forever, with no backoff cap and no diagnosis

**Tool:** kopi-beans (`bn`)
**Version:** `bn 0.1.3` (`bn --version`)
**Platform:** Windows 11 Pro 10.0.26200, Git Bash + PowerShell, `git` 2.x with `credential.helper=manager`
**Observed:** 2026-08-13, in the OUTRAM PARK backend workspace
**Status:** open, not yet filed upstream (no `gh` on this machine — see "Filing" below)

## Summary

The sync daemon retries a failing `git push` indefinitely at a fixed short
interval. There is no attempt cap, no escalating backoff that reaches a resting
state, and no surfaced reason for the failure. On Windows each retry spawns a
console-subsystem `git.exe` with no parent console, so **every retry allocates a
new console window that flashes on screen** — the workspace became unusable
until the daemon was killed by hand.

Worse: **the same push succeeds when run manually.** So the daemon is failing
for a reason specific to its own execution environment, and reporting nothing
about it.

## What I ran

Observing spawn rate over a 20-second window with the workspace completely idle
— no build running, no `bn` command issued:

```
$ # PowerShell: poll for newly-created processes for 20s
New processes spawned during a 20s idle window:
  conhost              9
  OpenConsole          9
  git                  8
```

Confirming the parent and the command line:

```
PARENT: bn.exe  x3
    "git" --git-dir=\\?\C:\...\outram-park-backend\.git
          --work-tree=\\?\C:\...\outram-park-backend
          push https://github.com/theodoreOnzGit/outram-park-backend
          refs/beads/03130356-213c-5905-a415-12b8110663e8/core:refs/beads/03130356-213c-5905-a415-12b8110663e8/core
```

`bn status`:

```
Sync:
  dirty:             true
  in_progress:       false
  last_sync:         never
  next_retry:        2026-08-13 07:54 (in 18.7s)
  consecutive_failures: 30
```

Note `next_retry` advertises ~20s, but the observed spawn rate was ~8 `git.exe`
in 20s — roughly one every 2.5s. Either multiple pushes are issued per retry, or
the retry interval is not what `bn status` reports.

## Phase 1 — a permanent failure retried as though transient

The push was being rejected non-fast-forward. Reproduced by hand with
`--dry-run` (writes nothing):

```
$ git push --dry-run origin refs/beads/03130356-.../core:refs/beads/03130356-.../core
To https://github.com/theodoreOnzGit/outram-park-backend
 ! [rejected]  refs/beads/03130356-.../core -> refs/beads/03130356-.../core (fetch first)
error: failed to push some refs to '...'
hint: Updates were rejected because the remote contains work that you do not
hint: have locally.
```

A non-fast-forward rejection is **not transient**. Retrying it unchanged can
never succeed — only fetching and reconciling can. The daemon retried it at
least 30 times and would have continued indefinitely.

The local and remote refs had genuinely diverged:

```
local ahead:   310 commits
remote ahead:  105,287 commits
```

The daemon never fetched, never reconciled, and never reported the divergence
through `bn status` — the only signal was `consecutive_failures` climbing.

## Phase 2 — still failing after the divergence was resolved

I resolved the divergence by hand (content diff showed local held 1212 issue ids
to the remote's 1086, with **zero** remote-only ids, so local was a strict
superset; the maintainer authorised publishing local upward). After that, local
was a clean fast-forward ahead of remote:

```
$ git push --dry-run origin refs/beads/03130356-.../core:refs/beads/03130356-.../core
To https://github.com/theodoreOnzGit/outram-park-backend
   29d79f57e9..cf389676a6  refs/beads/03130356-.../core -> refs/beads/03130356-.../core
```

That is a clean fast-forward — and pushing it for real from the same shell
succeeded immediately.

**The daemon, pushing the same refspec from the same repository as the same
user, still reported `consecutive_failures: 6`.** So whatever is failing is not
the ref state. The most likely candidate is that `credential.helper=manager`
(Git Credential Manager) cannot operate in the daemon's spawned environment —
it has no console and no interactive session to prompt from — but the daemon
surfaces no error text, so this is inference, not a confirmed diagnosis.

## Expected behaviour

1. **Classify failures.** A non-fast-forward rejection and an auth failure are
   permanent until something changes; they should stop the retry loop and mark
   the sync as needing intervention, not be retried on a transient-error timer.
2. **Cap the retries / back off to a resting interval.** Unbounded retry at a
   few seconds is a busy-loop, not a sync strategy.
3. **Surface the actual error.** `bn status` shows `consecutive_failures: 30`
   but never says *why*. The underlying `git` stderr should be captured and
   shown — that single line would have made this a two-minute diagnosis.
4. **Detect and report divergence.** `last_sync: never` plus a silently diverged
   ref is a data-loss trap: a user who "fixes" it by force-pushing the wrong
   direction destroys work. `bn status` should say the ref has diverged.
5. **Windows: spawn `git` without allocating a console.** Pass
   `CREATE_NO_WINDOW` when spawning child processes. Even with the retry bug
   fixed, ordinary sync activity should not flash console windows.

## Impact

- Workspace unusable on Windows due to constant console-window flashing;
  resolved only by `Stop-Process -Name bn -Force`.
- Sustained CPU and network load — a `git push` to GitHub every ~2.5s,
  indefinitely. This is plausibly the **root cause** of the separately filed
  [`kopi-beans-daemon-burns-37-percent-cpu-continuously.md`](./kopi-beans-daemon-burns-37-percent-cpu-continuously.md),
  which recorded 36.9% CPU sustained over 79 minutes on Linux without
  identifying a cause. Same daemon, same period, and a busy retry loop would
  produce exactly that profile. They may be one bug.
- `last_sync: never` meant every bead filed on this machine existed nowhere
  else — 126 issues, discovered only by diffing the refs by hand.

## Filing

`gh` is not installed on this machine (checked in both Git Bash and
PowerShell), so this is queued here per the workspace `CLAUDE.md` fallback
rather than filed upstream. To file it:

```bash
gh issue create --repo theodoreOnzGit/kopitiam \
  --title "kopi-beans: daemon retries a failing sync push forever, with no backoff cap and no diagnosis" \
  --body-file docs/kopitiam-issues/kopi-beans-daemon-retries-failing-push-forever.md
```

Related open issues in this queue:
[`kopi-beans-daemon-burns-37-percent-cpu-continuously.md`](./kopi-beans-daemon-burns-37-percent-cpu-continuously.md)
(likely same root cause) and
[`kopi-beans-daemon-sync-fails-slotmap-too-small.md`](./kopi-beans-daemon-sync-fails-slotmap-too-small.md)
(another sync failure mode; whether it is the error text behind this one is
unconfirmed).
