# kopi-beans: daemon sync fails with a `gix` "slotmap turned out to be too small" fetch error, and `bn sync` then hangs

> **UPSTREAMED 2026-08-13 as [kopitiam#27](https://github.com/theodoreOnzGit/kopitiam/issues/27).** Still OPEN upstream, so this file stays in the live queue rather than moving to `resolved/` — that move requires the fix to be verified against a published binary, not merely filed.

**Tool:** `kopi-beans` (binary `bn`)
**Version:** `bn 0.1.3` (installed via `cargo install kopi-beans`, 2026-08-12)
**Environment:** Linux 6.18.5, x86_64, remote-execution container. Repository
`theodoreOnzGit/outram-park-backend`, store `refs/heads/beads/store` at
`format_version 2` (~833 issues). 74 git refs total, of which 64 are
`refs/beads/backup/*`. `.git/packed-refs` absent — all refs loose.

## What I ran

Created 19 beads with explicit ids, then asked for a flush:

```
bn create --id op-0xv --title "…" -d "…" -t task -p 2      # x19, all exit 0
bn sync
```

## Observed

`bn create` succeeded 19/19 and the beads are immediately readable:

```
$ bn show op-0xv
op-0xv: fracture: FV vs FEM accuracy at the crack-tip singularity is an open research question
Status: Todo
Priority: P2
```

But they never reach the store ref. Before and after the creates:

```
$ git cat-file -p refs/heads/beads/store:state.jsonl | wc -l
832
$ git cat-file -p refs/heads/beads/store:state.jsonl | grep -c '"op-0xv"'
0
$ git rev-parse --short refs/heads/beads/store
2f9d6cd4e8      # unchanged across all 19 creates
```

`bn status` shows the daemon failing repeatedly:

```
Sync:
  next_retry:       2026-08-12 01:34 (in 17.8s)
  consecutive_failures: 77
  warnings:
    fetch_error: The slotmap turned out to be too small with 35 entries, would need 2 more (at 2026-08-12 00:49)
```

`bn sync` does not return — killed after 120 s:

```
$ bn sync
(no output; terminated at 120s, exit 143)
```

## Expected

Either:

1. `bn sync` flushes pending WAL entries to `refs/heads/beads/store` and
   returns; or
2. it fails **loudly and promptly** with an actionable error.

A silent hang is the worst of the three outcomes, because the caller has no
signal that the data is still only in the WAL.

## Why this matters more than a slow sync

`bn create` reports success and `bn show` confirms the bead, so from the CLI's
point of view the write landed. It has not landed anywhere durable: the WAL
lives under `/root/.local/share/`, **outside the repository**. In a
remote-execution container — which is where this workspace runs, and which had
already been restarted once that day, destroying everything not committed to
git — that means an apparently-successful `bn create` can be silently lost.

The 19 beads in question were *already* casualties of the related
[kopitiam#19](https://github.com/theodoreOnzGit/kopitiam/issues/19) (`bd`
cannot push the store ref); they had to be exported by hand and re-filed. They
are now at risk a second time from a different failure in the same path.

## Notes on the fetch error itself

`The slotmap turned out to be too small with 35 entries, would need 2 more`
appears to come from `gix`, not from kopi-beans directly. Two observations that
may or may not be relevant, offered as data rather than diagnosis:

- The repository has 74 refs, 64 of them `refs/beads/backup/*`, all loose (no
  `packed-refs`). The "35 entries" in the message does not obviously correspond
  to either count, so I did not pursue ref pressure as the cause.
- The error is recorded as a **fetch** error, yet what is blocked is a local
  **flush** to a local ref. If the flush is sequenced behind a fetch, a
  remote-side failure would block purely local durability — which would explain
  why creates never reach the ref. Worth checking whether that ordering is
  intended.

## Suggested handling, in priority order

1. **Do not let a fetch failure block the local flush.** Writing the WAL to
   `refs/heads/beads/store` needs no network; making local durability depend on
   remote reachability is the root harm here.
2. **`bn sync` must not hang.** Give it a timeout and a non-zero exit.
3. **Surface the risk in `bn status`** — something like "N entries pending,
   not yet in the store ref" would have made this visible immediately instead
   of requiring a `git cat-file` to discover.

## Filed locally because

`gh` is not available in this session and the kopitiam repository is not in
this session's GitHub scope, so this is queued here for upstreaming per the
workspace `CLAUDE.md` "Raising issues" rule. Everything above is transcribed
from commands actually run on 2026-08-12; no output is reconstructed.

---

# UPDATE 2026-08-13 — a worse symptom: silent data loss AFTER reported success

> **Upstreamed separately as [kopitiam#28](https://github.com/theodoreOnzGit/kopitiam/issues/28)**, because it is a different defect from the sync hang above: there the data survived in the WAL, here the daemon reports `dirty: false` and the records are gone.

The original report above was about `bn sync` hanging and creates not reaching
the store ref. That was recoverable — the data was still in the WAL and the
daemon eventually flushed it. **This update records a harder failure: beads that
were created, used, and in one case explicitly closed, and have since vanished
from a store that believes itself fully synced.**

## What was lost

Three beads filed by agents during this session are gone:

| id | filed for | evidence it existed |
|---|---|---|
| `op-ad6h` | the `ode::normalize_error` NaN defect | **`bn close op-ad6h` printed `✓ Closed op-ad6h: Done`** |
| `op-zwk0` | the same defect in two vendored ODE trees | an agent reported filing it; a later agent could not `bn show` it |
| `op-3ep6` | GPU fixed-rule quadrature | filed and reported by the `op-yvj.4.5` agent |

## The state that makes this a defect rather than a pending flush

```
$ bn show op-ad6h
 ERROR error: bead not found: op-ad6h

$ git cat-file -p refs/heads/beads/store:state.jsonl | grep -c '"op-ad6h"'
0
$ git cat-file -p refs/remotes/origin/beads/store:state.jsonl | grep -c '"op-ad6h"'
0

$ bn status
  Total Issues:      956
  dirty:             false
  last_sync:         2026-08-13 05:09
```

`dirty: false` with a recent `last_sync` means the daemon considers everything
written. It is not waiting to flush; it has concluded it is done. The beads are
absent from bn's index, the local ref, and origin.

Beads filed EARLIER in the same session by a different agent (`op-uyi3`,
`op-px11`, `op-fwe7`, `op-jwer`) are all present. So this is not "the store is
broken" — it is selective loss of a later window of writes.

## Why this is the serious one

`bn close op-ad6h` returned success. A tracker that acknowledges a state
transition on a record it will later not have is worse than one that refuses,
because the caller has no signal to retry. Every earlier symptom in this file
at least left the data in the WAL.

Consequence for this repository: three code comments and several commit
messages now cite bead ids that do not resolve. The content was preserved in
commit messages instead, because git has been the only reliable storage in this
environment.

## Suggested handling, in priority order

1. **Never acknowledge a mutation that is not durable.** `bn create` and
   `bn close` should not return success until the write is recoverable.
2. **`dirty: false` must mean it.** If writes can be dropped between
   acknowledgement and sync, the status flag is actively misleading.
3. A `bn fsck`-style reconciliation that reports acknowledged-but-absent ids
   would at least make the loss detectable rather than discovered by a later
   `bn show` failing.

Everything above is transcribed from commands actually run on 2026-08-13.
