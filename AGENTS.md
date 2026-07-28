# Agent Instructions

This project uses **beads-rs** (`bd`) for issue tracking. Run `bd prime` for full workflow context.

> **RUST BEADS ONLY.** Beads here is the **Rust** implementation — the
> [`beads-rs`](https://crates.io/crates/beads-rs) crate
> (`github.com/delightful-ai/beads-rs`), binary `bd`. Do **not** install or use
> any Go build of beads. If only a Go `bd` is on this machine, treat beads as
> unavailable and fall back to a short markdown note rather than filing into it.
>
> **Tooling note (2026-07-20):** this workspace migrated off the Go beads/Dolt
> implementation to **beads-rs** (`cargo install beads-rs`, binary `bd`). The
> old Go binary is parked at `~/.local/bin/bd-go.deprecated`; `bd` now resolves
> to beads-rs. All 335 beads were migrated via `bd migrate from-go`.
>
> **Architecture in one line (beads-rs):** issues live **in git refs** —
> canonical state on `refs/heads/beads/store` (files `state.jsonl`,
> `deps.jsonl`, `tombstones.jsonl`, `meta.json` inside that ref), backups under
> `refs/beads/backup/*`. A background **`bd daemon`** debounces and
> **auto-syncs** (push/pull) the `beads/store` ref to your git remote — a
> private channel, separate from `refs/heads/*` where your code lives. There is
> **no `bd dolt`** and **no `.beads/` Dolt DB**; `.beads/issues.jsonl` is now a
> local compat-export **symlink**, not the source of truth.
>
> The daemon's auto-sync of `beads/store` is beads-rs's designed, opted-in
> behavior — it does **not** relax the workspace rule against committing/pushing
> **code** branches (`refs/heads/*`) without explicit approval.

## Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd claim <id>         # Claim work
bd close <id>         # Complete work
bd sync               # Wait for the background git sync to flush
# sync is automatic via `bd daemon`; `bd sync` only blocks until it lands
```

## Non-Interactive Shell Commands

**ALWAYS use non-interactive flags** with file operations to avoid hanging on confirmation prompts.

Shell commands like `cp`, `mv`, and `rm` may be aliased to include `-i` (interactive) mode on some systems, causing the agent to hang indefinitely waiting for y/n input.

**Use these forms instead:**
```bash
# Force overwrite without prompting
cp -f source dest           # NOT: cp source dest
mv -f source dest           # NOT: mv source dest
rm -f file                  # NOT: rm file

# For recursive operations
rm -rf directory            # NOT: rm -r directory
cp -rf source dest          # NOT: cp -r source dest
```

**Other commands that may prompt:**
- `scp` - use `-o BatchMode=yes` for non-interactive
- `ssh` - use `-o BatchMode=yes` to fail instead of prompting
- `apt-get` - use `-y` flag
- `brew` - use `HOMEBREW_NO_AUTO_UPDATE=1` env var

<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:970c3bf2 -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd claim <id>         # Claim work
bd close <id>         # Complete work
```

### Rules

- Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists
- Run `bd prime` for detailed command reference and session close protocol
- Use `bd remember` for persistent knowledge — do NOT use MEMORY.md files

**Architecture in one line (beads-rs):** issues live in git refs — canonical state on `refs/heads/beads/store` (state/deps/tombstones jsonl + meta.json), backups under `refs/beads/backup/*`; a background `bd daemon` auto-syncs that ref to your git remote (separate from `refs/heads/*` code branches). No `bd dolt`, no `.beads/` Dolt DB; `.beads/issues.jsonl` is a local compat-export symlink, not the source of truth. Migrated off Go beads on 2026-07-20.

## Agent Context Profiles

The managed Beads block is task-tracking guidance, not permission to override repository, user, or orchestrator instructions.

- **Conservative (default)**: Use `bd` for task tracking. Do not run git commits, git pushes, or a manual beads sync (`bd sync`) unless explicitly asked. At handoff, report changed files, validation, and suggested next commands.
- **Minimal**: Keep tool instruction files as pointers to `bd prime`; use the same conservative git policy unless active instructions say otherwise.
- **Team-maintainer**: Only when the repository explicitly opts in, agents may close beads, run quality gates, commit, and push as part of session close. A current "do not commit" or "do not push" instruction still wins.

## Session Completion

This protocol applies when ending a Beads implementation workflow. It is subordinate to explicit user, repository, and orchestrator instructions.

1. **File issues for remaining work** - Create beads for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **Handle git/sync by active profile**:
   ```bash
   # Conservative/minimal/default: report status and proposed commands; wait for approval.
   git status

   # Team-maintainer opt-in only, unless current instructions forbid it:
   git pull --rebase
   bd sync            # optional: block until the `bd daemon` flush lands
   git push
   git status
   ```
5. **Hand off** - Summarize changes, validation, issue status, and any blocked sync/commit/push step

**Critical rules:**
- Explicit user or orchestrator instructions override this Beads block.
- Do not commit or push without clear authority from the active profile or the current user request.
- If a required sync or push is blocked, stop and report the exact command and error.
<!-- END BEADS INTEGRATION -->

<!-- BEGIN BEADS CODEX SETUP: generated by bd setup codex -->
## Beads Issue Tracker

Use Beads (`bd`) for durable task tracking in repositories that include it. Use the `beads` skill at `.agents/skills/beads/SKILL.md` (project install) or `~/.agents/skills/beads/SKILL.md` (global install) for Beads workflow guidance, then use the `bd` CLI for issue operations.

### Quick Reference

```bash
bd ready                # Find available work
bd show <id>            # View issue details
bd claim <id>           # Claim work
bd close <id>           # Complete work
bd prime                # Refresh Beads context
```

### Rules

- Use `bd` for all task tracking; do not create markdown TODO lists.
- Run `bd prime` when Beads context is missing or stale. Codex 0.129.0+ can load Beads context automatically through native hooks; use `/hooks` to inspect or toggle them.
- Keep persistent project memory in Beads via `bd remember`; do not create ad hoc memory files.

**Architecture in one line (beads-rs):** issues live in git refs — canonical state on `refs/heads/beads/store` (state/deps/tombstones jsonl + meta.json), backups under `refs/beads/backup/*`; a background `bd daemon` auto-syncs that ref to your git remote (separate from `refs/heads/*` code branches). No `bd dolt`, no `.beads/` Dolt DB; `.beads/issues.jsonl` is a local compat-export symlink, not the source of truth. Migrated off Go beads on 2026-07-20.
<!-- END BEADS CODEX SETUP -->
