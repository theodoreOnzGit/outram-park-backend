# kopi-beans — AI-Native Issue Tracking

This repository uses **kopi-beans** for issue tracking — issues that live
directly in the repo alongside the code, driven entirely from the CLI.

> **KOPI-BEANS ONLY, as of 2026-08-07.** This project previously used
> **beads-rs** ([crates.io/crates/beads-rs](https://crates.io/crates/beads-rs),
> binary `bd`); per explicit maintainer instruction it now uses **kopi-beans**
> (binary `bn`), a Windows/Termux-capable fork of beads-rs published to
> [crates.io/crates/kopi-beans](https://crates.io/crates/kopi-beans)
> (upstream: [github.com/theodoreOnzGit/kopitiam](https://github.com/theodoreOnzGit/kopitiam)).
> Do **not** install or use `beads-rs`/`bd` in this workspace anymore.

> **BLOCKER — live as of 2026-08-07.** `bn` (kopi-beans 0.1.1) cannot read the
> `refs/heads/beads/store` ref this repo already has from `bd`. Every command
> that touches the store (`bn init`, `bn status`, `bn list`, `bn ready`, …)
> fails with:
>
> ```
> ERROR error: invalid field value: unsupported meta format_version 1
> ```
>
> Filed upstream as
> [kopitiam#16](https://github.com/theodoreOnzGit/kopitiam/issues/16) (already
> open, filed by the maintainer 2026-08-06). The existing store (~335 issues)
> is left untouched — **do not delete `refs/heads/beads/store`, and do not
> hand-patch its `meta.json` to force compatibility.** Until upstream fixes
> this, `bn` has no working store here; fall back to TodoWrite/TaskCreate or a
> short markdown note, and say so in your hand-off.

## What is kopi-beans?

kopi-beans is issue tracking that lives in your repo — no web UI, everything
through the `bn` CLI, designed to work well with AI coding agents. Issue data
is stored as JSONL on a dedicated git branch and synced in the background
over git. Its CLI mirrors beads-rs's `bd` command-for-command (`init`,
`create`, `show`, `list`, `ready`, `claim`, `close`, `dep`, `status`,
`prime`, …) since it's a fork of the same lineage.

## Install

```bash
cargo install kopi-beans    # installs the `bn` binary to ~/.cargo/bin
```

First-time setup in a fresh clone (once the blocker above is resolved):

```bash
bn init
bn onboard
```

**Do not run `bn setup claude`** — as of kopi-beans 0.1.1 it writes a Claude
Code hook whose command is literally `bd prime`, not `bn prime` (a real
branding bug, not a typo in this README — verified 2026-08-07 by running it in
a scratch repo). The hook it installs would fail outright since only `bn` is
on `PATH`. Hand-maintain `.claude/settings.json`'s hook instead; the working
equivalent of `bd`'s `--hook-json` flag (which `bn prime` does not have) is
`bn prime --mcp`.

## Quick Start (once the blocker above is resolved)

```bash
# Create new issues
bn create "Add user authentication"

# View all issues
bn list

# View issue details
bn show <issue-id>

# Update issue status
bn update <issue-id> --claim
bn update <issue-id> --status done

# Wait for the background git sync to flush
bn sync
```

## How it works

- **Git-native**: issues are stored as JSONL on a dedicated git branch
  (`refs/heads/beads/store`) — canonical state in `state.jsonl`,
  `tombstones.jsonl`, `deps.jsonl`, `meta.json`. There is **no Dolt database
  and no SQLite**. This is the exact same ref name and file layout beads-rs
  used, which is exactly why the two collide (see the blocker above).
- **Background sync**: mutations are debounced and pushed in the background by
  a local daemon that starts on demand; `bn sync` just waits for the flush.
- **Passive export**: `.beads/issues.jsonl` is a passive export, not the source
  of truth — don't treat it as authoritative and don't import it during
  normal operation.
- **AI-friendly**: CLI-first, no context switching to a web UI. Note that some
  of `bn`'s own generated text (`bn prime`, `bn ready`'s footer) still says
  `bd` internally — a known kopi-beans 0.1.1 branding bug, not an instruction
  to use `bd`.

## Learn More

- **Crate**: [crates.io/crates/kopi-beans](https://crates.io/crates/kopi-beans)
- **Source**: [github.com/theodoreOnzGit/kopitiam](https://github.com/theodoreOnzGit/kopitiam)
  (read-only reference only — see the workspace `CLAUDE.md` "CONSUME THE
  BINARIES ONLY" rule; never edit or vendor it from this workspace)
- **Full workflow context**: run `bn prime`
- **Known-friction tracking**: `docs/kopitiam-issues/` in this workspace, plus
  the upstream issue linked above
