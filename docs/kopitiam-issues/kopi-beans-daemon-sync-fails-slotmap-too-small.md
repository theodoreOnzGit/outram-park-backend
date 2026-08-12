# kopi-beans: daemon sync fails with a `gix` "slotmap turned out to be too small" fetch error, and `bn sync` then hangs

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
