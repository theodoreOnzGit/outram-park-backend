# kopi-beans: the daemon commits a checkpoint every ~7.5 s, never lands the push, and leaves ~145 k unreachable git objects behind

**Tool:** `kopi-beans` (binary `bn`)
**Version:** `bn 0.1.9` (`cargo install --list` → `kopi-beans v0.1.9`), 2026-09-14
**Environment:** Linux 6.18.44, x86_64, remote-execution container.
Repository `theodoreOnzGit/outram-park-backend`, checked out **as a git
submodule**, so its git dir is `<parent>/.git/modules/outram-park-backend`.

> **Supersedes an earlier, wrong write-up of the same session.** This file was
> first filed as *"`bn sync` fails … and the write is silently stranded"*, on
> the strength of `refs/heads/beads/store` not moving. **That diagnosis was
> incorrect**, and the correction is the useful part of this report — see
> "What I got wrong" below. The underlying problem is real, but it is bigger
> and in a different place than first described.

## Summary

The daemon writes a `beads checkpoint core <hash>` commit roughly **every 7.5
seconds**, continuously. It has never successfully pushed them. The result is
a local ref 40 402 commits deep whose remote copy is **39 590 commits behind**,
and a git object store carrying **~145 000 unreachable objects** — which in
turn trips git's *"too many unreachable loose objects"* warning, writes
`gc.log`, and thereby **disables automatic gc**, so nothing ever cleans up.

## Measurements

```
$ git log -3 --format='%h %ad %s' --date=format:'%H:%M:%S' \
      refs/beads/03130356-213c-5905-a415-12b8110663e8/core
8f89dca8d 09:53:42 beads checkpoint core fb198bba68…
7c81965b0 09:53:35 beads checkpoint core 000e390346…      # 7 s earlier
8540262c9 09:53:27 beads checkpoint core 34826af85f…      # 8 s earlier

$ git rev-list --count --since='1 hour ago' refs/beads/…/core
440                                    # ~10 500/day

$ git rev-list --count refs/beads/…/core
40402

$ git rev-list --count refs/remotes/origin/bn-core..refs/beads/…/core
39590                                  # remote is this far behind

$ git fsck --unreachable --no-reflogs | awk '{print $2}' | sort | uniq -c
  49695 blob
  47721 tree
  47448 commit                          # ~1:1:1 — one shard per checkpoint

$ git count-objects -vH
count: 24726          size: 149.43 MiB
in-pack: 209683       packs: 34         size-pack: 330.84 MiB
```

Every sampled unreachable commit is a daemon checkpoint:

```
$ git log -1 --format='%an <%ae>%n%s' 0600e00f3
beads <beads@localhost>
beads checkpoint core 230dda90fb43a4ce771a78dade1b26346e78c653066b4fd34e9dd31a49d613f0
```

## Why this is worse than it looks

The failure is **silent at every point a user would check**. `bn update` exits
0. `bn show` reads the note back correctly. And the documented manual fallback

```
$ git push origin refs/heads/beads/store:refs/heads/beads/store
Everything up-to-date
```

reports success — truthfully, because 0.1.9 no longer writes the data there.
Nothing surfaces the fact that the work is not leaving the machine unless you
compare the per-machine ref against its remote by hand.

`bn sync` does fail, but its message points away from the real problem:

```
ERROR error: sync wait gave up after 6183ms for github.com/theodoreOnzGit/outram-park-backend:
git operation failed: An error occurred when trying to resolve an object a reference points to
```

## What I got wrong, and why it matters for the fix

I concluded the write "was never committed" because `refs/heads/beads/store`
did not move. It had in fact been committed — to
`refs/beads/<uuid>/core`, the per-machine ref that 0.1.9 actually uses.
`refs/heads/beads/store` is now a *legacy* ref that no longer receives writes,
while the remote carries three beads refs:

```
refs/beads/03130356-213c-5905-a415-12b8110663e8/core
refs/beads/meta
refs/heads/beads/store
```

This matters twice over. First, the OUTRAM PARK workspace `CLAUDE.md` documents
`refs/heads/beads/store` as canonical and prescribes exactly one fallback
refspec, so **the documented recovery procedure is now a no-op** — it pushes a
ref that is no longer written. Second, any diagnostic that watches the old ref
will conclude the tracker is idle when it is in fact writing 10 500 commits a
day. Whatever the fix, the ref-layout change needs to be surfaced somewhere a
user will meet it.

## Related

- [`kopi-beans-daemon-burns-37-percent-cpu-continuously.md`](./kopi-beans-daemon-burns-37-percent-cpu-continuously.md)
  (kopitiam#26, open). A checkpoint-plus-failed-push cycle every 7.5 s is a
  plausible cause of exactly that CPU profile.
- [`resolved/kopi-beans-daemon-retries-failing-push-forever.md`](./resolved/kopi-beans-daemon-retries-failing-push-forever.md)
  — "daemon retries a failing push roughly every 2.5 s indefinitely", recorded
  as **fixed in 0.1.6**. What is observed here on **0.1.9** has the same shape
  at a longer interval, and now also commits on each cycle. Worth checking
  whether that regression has returned or was only partly fixed.
- [`kopi-beans-daemon-sync-fails-slotmap-too-small.md`](./kopi-beans-daemon-sync-fails-slotmap-too-small.md)
  (kopitiam#27, open) — a different `gix` error on 0.1.3, same consequence.

## Expected

1. The daemon should not commit when nothing has changed, and should back off
   rather than checkpoint-per-cycle when the push is failing.
2. A write that cannot be published should say so at `bn update` time, not
   only inside `bn sync`'s error.
3. If the canonical ref has moved, `bn` should say which ref it is using.

## Remediation applied here (not a fix)

`git prune --expire=1.hour.ago` then `git gc --prune=1.hour.ago`, with an
expiry window because the daemon is live and writing; `gc.log` removed so
automatic gc can resume. This reclaims the garbage but does nothing about the
rate at which it is produced.

## Upstreaming

Not yet filed on `theodoreOnzGit/kopitiam` — that repository is outside this
session's GitHub scope. **This file is the live queue entry; upstream it and
add the issue link at the top**, as the two files above do.
