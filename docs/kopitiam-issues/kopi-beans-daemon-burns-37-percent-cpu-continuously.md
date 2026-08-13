# kopi-beans: `bn daemon run` holds ~37% of a CPU core continuously

**Tool:** kopi-beans (`bn`)
**Version:** see `cargo install --list` output recorded below
**Observed:** 2026-08-13, in the OUTRAM PARK backend workspace
**Status:** open, not yet filed upstream (no `gh` in this session — see below)

## What I ran

Nothing in particular — the daemon is started by ordinary `bn` use. The
observation came from checking machine load before recording benchmark
timings for bead `op-yvj.4.6`:

```
$ ps -eo pid,pcpu,etime,args --sort=-pcpu | head -3
  PID %CPU     ELAPSED COMMAND
 8008 36.9    01:19:29 /root/.cargo/bin/bn daemon run
14857 18.7       00:00 /bin/bash -c ...
  511  5.3    01:24:27 claude ...
```

```
$ uptime
 03:06:33 up  1:24,  0 user,  load average: 3.33, 2.06, 1.49
```

## Observed behaviour

`bn daemon run` averaged **36.9% CPU over 79 minutes of elapsed time**. That is
not a burst during a write — it is a sustained average across the whole session,
on a machine with **4 logical cores**, so the daemon alone accounts for roughly
9% of total machine capacity and pushes the 1-minute load average above 2 with
no other work running.

The workspace was otherwise idle for most of that window: no builds were running
during the sampling above, and the beads store had not been written to for
several minutes.

## Expected behaviour

A debouncing file-watch daemon should be near 0% CPU when nothing is changing.
A steady ~37% suggests a busy-wait poll loop rather than a blocking watch, or a
watch that re-scans the store on every tick.

## Why it matters here beyond tidiness

This workspace's CLAUDE.md requires per-kernel CPU crossover benchmarks with
measured numbers (`op-yvj.4.7` and every `op-yvj.4.x` child). A constant ~1 core
of background load on a 4-core box materially perturbs those measurements —
`rayon` speed-ups recorded in `crates/outram-foam-basic-lib/src/math/differentiate.rs`
had to be qualified with "the machine was NOT idle" for exactly this reason, and
two independent runs of the same benchmark disagreed by up to 2x on the parallel
column. The benchmarks are honest about it, but the noise is avoidable.

## Not verified

- Whether CPU use is proportional to store size (this store has ~600+ issues).
- Whether it drops when no repository is being watched.
- Whether it reproduces on a quiescent machine outside a container.

Someone upstreaming this should confirm those and attach the exact version:

```bash
cargo install --list | grep -A1 kopi-beans
```

## Filing channel

Per CLAUDE.md the preferred channel is a GitHub issue on
`theodoreOnzGit/kopitiam`. This session has no `gh` CLI (GitHub access is via
MCP tools scoped to `theodoreonzgit/outram-park-backend` only), so this is filed
locally as the documented fallback. **It still needs upstreaming.**
