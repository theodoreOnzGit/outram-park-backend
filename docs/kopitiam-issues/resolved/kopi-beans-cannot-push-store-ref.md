# kopi-beans: `bn` could not push the store ref to a non-local remote

**Upstream issue:** [kopitiam#19](https://github.com/theodoreOnzGit/kopitiam/issues/19)
**Status: RESOLVED — verified 2026-08-12 against kopi-beans v0.1.3.**

> This file was created *at resolution time*. The issue was filed directly on
> GitHub and never had a local file in the `docs/kopitiam-issues/` queue, so
> there was nothing to move; per the workspace `CLAUDE.md` rule that resolved
> issues live in `resolved/` with their closing evidence, the record is written
> here instead.

## Original report

`bn` drives git through `gix`, which has no send-pack implementation. The
consequence recorded in this workspace was that `bn` could update
`refs/heads/beads/store` locally but could not publish it to `origin`, so the
daemon's auto-sync to GitHub did not work and `bn status` would report
`last_sync: never` indefinitely.

The documented workaround was a manual push with real `git`:

```bash
git push origin refs/heads/beads/store:refs/heads/beads/store
```

and, from 2026-08-11, a `Stop` hook in `.claude/settings.json` running
`./scripts/push-beads-store.sh` to do that automatically at the end of each
session turn.

## Why this was previously "unverified" rather than resolved

On 2026-08-11 this workspace observed `bn status` reporting a real `last_sync`
timestamp with the remote ref already matching the local one — but a manual push
had also been run in that same session, so it was impossible to attribute the
match to the daemon rather than to the manual push. That ambiguity is exactly
why the workspace recorded #19 as **unverified rather than resolved** and
declined to restate either claim as fact.

## Closing evidence (2026-08-12, kopi-beans v0.1.3)

The ambiguity was removed by making a store mutation and then checking the
remote **without running the push script at any point afterwards**.

Version under test:

```
$ cargo install --list | grep -A1 kopi-beans
kopi-beans v0.1.3:
    bn
```

Remote ref before the mutation:

```
$ git ls-remote origin refs/heads/beads/store | cut -f1
d383a8e78067f83f122b0e5f5d41da88b2085c56
```

Mutation — creating a real work item (`op-szmi.9`):

```
$ bn create "fhr_sim_v2: GUI clones the whole FHRState twice per frame" \
    --type=task --priority=2 --parent=op-szmi -d "..."
✓ Created issue: op-szmi.9
```

Local and remote a short time later, **with no manual push in between**:

```
$ git rev-parse refs/heads/beads/store
ec04ac1f28ea97d811408148c37c92ee4f5ee944
$ git ls-remote origin refs/heads/beads/store | cut -f1
ec04ac1f28ea97d811408148c37c92ee4f5ee944
```

The ref advanced from `d383a8e` to `ec04ac1` **on the GitHub remote**, published
by the kopi-beans daemon alone. Corroborating daemon state:

```
$ bn status
Sync:
  dirty:             false
  in_progress:       false
  last_sync:         2026-08-12 04:07
  consecutive_failures: 0

$ pgrep -a -x bn
388777 /home/teddy0/.cargo/bin/bn daemon run
```

**Interpretation:** kopi-beans v0.1.3 publishes `refs/heads/beads/store` to a
non-local git remote without assistance. `last_sync: never` is no longer the
expected steady state, and the manual push is no longer required for the store
to reach GitHub.

## Consequences for this workspace

- The `Stop` hook running `./scripts/push-beads-store.sh` is now **redundant**.
  It remains harmless — the script is idempotent and a no-op when the local ref
  already matches the remote — but it is no longer load-bearing.
- Retiring that hook would also retire the standing exception to the
  never-auto-push rule that was introduced solely to work around this bug. That
  is a maintainer policy decision, not an automatic consequence of this
  verification, and `CLAUDE.md` should not be edited to drop the carve-out until
  the maintainer decides to.
- The manual push remains valid and idempotent, and is still the correct
  fallback if the daemon is not running.

## Still to do upstream

The GitHub issue [kopitiam#19](https://github.com/theodoreOnzGit/kopitiam/issues/19)
was still **open** at the time of this verification. Per the workspace rule that
the closing of an upstream issue is the maintainer's action, it has not been
closed from this workspace — a verification comment carrying this evidence is
the extent of the involvement here.
