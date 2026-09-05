---
name: beads
description: Use when working in a repository that uses bn (kopi-beans) for durable project task tracking, issue dependencies, blocker management, multi-session handoff, or shared work memory. Trigger when the user asks to find ready work, claim or close tasks, create follow-up work, inspect blockers, recover project context, or choose between local planning and persistent project tracking. NOTE: as of 2026-08-07 this workspace's kopi-beans store cannot be read (see "Live blocker" below) — check that section before assuming bn commands will work.
---

# kopi-beans

> **Migrated 2026-08-07** from beads-rs (`bd`) to kopi-beans (`bn`), per
> explicit maintainer instruction. This skill file (and its directory name,
> `.agents/skills/beads/`) still says "beads" — that's the skill's identifier,
> not an instruction to use `bd`. Do **not** install or use `beads-rs`/`bd` in
> this workspace anymore.

**Live blocker (as of 2026-08-07):** `bn` (kopi-beans 0.1.1) cannot read this
repo's existing `refs/heads/beads/store` ref — every store-touching command
(`bn init`, `bn status`, `bn list`, `bn ready`, …) fails with `unsupported
meta format_version 1`. Filed upstream as
[kopitiam#16](https://github.com/theodoreOnzGit/kopitiam/issues/16). Until
that's fixed, treat the tracker as unavailable: use TodoWrite/TaskCreate or a
short markdown note instead, and say so in your hand-off. The rest of this
file describes the intended workflow for once it's unblocked.

Use kopi-beans as the shared project task system. Local plans, scratch files, and personal memories are useful, but they are not the durable source of truth for project work.

## First Step

Run:

```bash
bn prime
```

If that prints nothing, check whether the repository has an active kopi-beans workspace:

```bash
bn status
```

(`bn where` is not a real subcommand — beads-rs's `bd where` does not exist on `bn`; `bn status` is the closest working equivalent, verified against `bn --help` 2026-08-07.)

## Preferred Route

Use the `bn` CLI when shell access is available. It is the most compact and direct kopi-beans interface.

## Core CLI Workflow

1. Find work:

```bash
bn ready
bn list --status=open
bn list --status=in_progress
```

2. Inspect before editing:

```bash
bn show <id>
```

3. Claim work atomically:

```bash
bn update <id> --claim
```

4. Create durable follow-up work when implementation reveals new tasks:

```bash
bn create "Short title" --description="Why this exists and what needs to be done" --type=task --priority=2
```

5. Close completed work:

```bash
bn close <id> --reason="Completed"
```

## What Belongs In kopi-beans

Use kopi-beans for:

- shared project tasks
- blockers and dependencies
- discovered follow-up work
- work that must survive thread reset, compaction, or handoff
- status that another person or agent should be able to resume

Use agent-local planning tools only for the current turn's execution checklist. Do not treat them as shared project state.

## Rules

- Do not create markdown TODO files as the source of truth when kopi-beans is available and its store is readable.
- Do not use `bn edit`; it opens an interactive editor. Use `bn update` flags instead.
- Prefer `--json` when parsing `bn` output programmatically.
- If hooks are installed, `bn prime` may already be injected. Run it manually when context is missing. **Do not run `bn setup claude`** to (re)install those hooks — as of kopi-beans 0.1.1 it writes a hook command that literally invokes `bd`, not `bn`, so the hook fails outright; hand-maintain `.claude/settings.json` instead.
- Do not auto-close or mutate tasks unless the work is actually complete.
