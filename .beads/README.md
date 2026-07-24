# Beads — AI-Native Issue Tracking

This repository uses **Beads** for issue tracking — issues that live directly
in the repo alongside the code, driven entirely from the CLI.

> **RUST BEADS ONLY.** This project uses the **Rust** implementation, the
> [`beads-rs`](https://crates.io/crates/beads-rs) crate
> (`github.com/delightful-ai/beads-rs`), binary `bd`. Do **not** install or
> use any Go build of beads.

## What is Beads?

Beads is issue tracking that lives in your repo — no web UI, everything through
the `bd` CLI, designed to work well with AI coding agents. Issue data is stored
as JSONL on a dedicated git branch and synced in the background over git.

## Install

```bash
cargo install beads-rs      # installs the `bd` binary to ~/.cargo/bin
```

First-time setup in a fresh clone:

```bash
bd init
bd setup claude
```

**Migrating the legacy store (one-time):** the pre-existing issues in
`.beads/issues.jsonl` were written by the old Go tool. Import them into
`beads-rs` with (dry-run first):

```bash
bd migrate from-go --input .beads/issues.jsonl --dry-run
bd migrate from-go --input .beads/issues.jsonl
```

## Quick Start

```bash
# Create new issues
bd create "Add user authentication"

# View all issues
bd list

# View issue details
bd show <issue-id>

# Update issue status
bd update <issue-id> --claim
bd update <issue-id> --status done

# Wait for the background git sync to flush
bd sync
```

## How it works

- **Git-native**: issues are stored as JSONL on a dedicated git branch
  (`refs/heads/beads/store`) — canonical state in `state.jsonl`,
  `tombstones.jsonl`, `deps.jsonl`, `meta.json`. There is **no Dolt database
  and no SQLite**.
- **Background sync**: mutations are debounced and pushed in the background by
  a local daemon that starts on demand; `bd sync` just waits for the flush.
- **Passive export**: `.beads/issues.jsonl` is a passive export, not the source
  of truth — don't treat it as authoritative and don't `bd import` during
  normal operation.
- **AI-friendly**: CLI-first, no context switching to a web UI.

## Learn More

- **Crate**: [crates.io/crates/beads-rs](https://crates.io/crates/beads-rs)
- **Source**: [github.com/delightful-ai/beads-rs](https://github.com/delightful-ai/beads-rs)
- **Full workflow context**: run `bd prime`
