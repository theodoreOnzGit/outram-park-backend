# Agent Instructions

This project uses **kopi-beans** (`bn`) for issue tracking — replacing
**beads-rs** (`bd`) as of 2026-08-07, per explicit maintainer instruction. Run
`bn prime` for workflow context.

> **KOPI-BEANS ONLY, going forward.** Do **not** install or use `beads-rs`
> (binary `bd`) in this workspace anymore. `bn` (kopi-beans) is the mandated
> tool; its CLI mirrors `bd`'s 1:1 (`init`, `create`, `show`, `list`, `ready`,
> `claim`, `close`, `dep`, `status`, `prime`, …). If only a Go `bd` build or a
> pre-migration `bd` install is on this machine, treat the tracker as
> unavailable and fall back to a short markdown note rather than filing into
> it.
>
> **BLOCKER, live as of 2026-08-07:** `bn` (kopi-beans 0.1.1) cannot read
> the `refs/heads/beads/store` ref this workspace's `bd` already wrote — every
> store-touching command (`bn init`, `bn status`, `bn list`, `bn ready`, …)
> fails with `unsupported meta format_version 1`. Filed upstream as
> [kopitiam#16](https://github.com/theodoreOnzGit/kopitiam/issues/16) (already
> open, filed by the maintainer 2026-08-06). The old ref and its ~335 issues
> are left untouched — do not delete it, do not hand-patch its `meta.json` to
> work around this. **Until it's fixed, there is no working CLI issue tracker
> in this repository** — use TodoWrite/TaskCreate or a short markdown note
> instead, and say so in your hand-off.
>
> **Architecture in one line (kopi-beans):** issues live **in git refs** —
> canonical state on `refs/heads/beads/store` (files `state.jsonl`,
> `deps.jsonl`, `tombstones.jsonl`, `meta.json` inside that ref), backups under
> `refs/beads/backup/*`. A background daemon debounces and **auto-syncs**
> (push/pull) the `beads/store` ref to your git remote — a private channel,
> separate from `refs/heads/*` where your code lives. Same ref/file layout as
> beads-rs (it's a fork), which is exactly why the two collide — see the
> blocker above. `.beads/issues.jsonl` is a local compat-export **symlink**,
> not the source of truth.
>
> The daemon's auto-sync of `beads/store` is kopi-beans's designed, opted-in
> behavior — it does **not** relax the workspace rule against committing/pushing
> **code** branches (`refs/heads/*`) without explicit approval.
>
> A prior migration, 2026-07-20, moved this workspace off a Go/Dolt
> implementation onto beads-rs; that history is superseded by this entry.

## Quick Reference (once the store above is readable)

```bash
bn ready              # Find available work
bn show <id>          # View issue details
bn claim <id>         # Claim work
bn close <id>         # Complete work
bn sync               # Wait for the background git sync to flush
# sync is automatic via the bn daemon; `bn sync` only blocks until it lands
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

<!-- BEGIN ISSUE TRACKER INTEGRATION (hand-maintained for kopi-beans since 2026-08-07; formerly BEADS INTEGRATION v:1 profile:minimal hash:970c3bf2) -->
## Issue Tracker (kopi-beans)

This project uses **kopi-beans** (`bn`) for issue tracking. Run `bn prime` to
see workflow context. **Note the live blocker above:** every command that
actually touches this repo's store currently fails with `unsupported meta
format_version 1` — see [kopitiam#16](https://github.com/theodoreOnzGit/kopitiam/issues/16).
Until that's fixed upstream, use TodoWrite/TaskCreate instead and note the gap
in your hand-off.

### Quick Reference

```bash
bn ready              # Find available work
bn show <id>          # View issue details
bn claim <id>         # Claim work
bn close <id>         # Complete work
```

### Rules

- Use `bn` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists, **except while the format-version blocker above is open**, in which case those are the working fallback.
- Run `bn prime` for detailed command reference and session close protocol.
- **`bn` has no `remember` command** (verified against `bn --help`, 2026-08-07) — unlike beads-rs's `bd remember`, there is no equivalent. Keep using per-project `memory/` + `MEMORY.md` files for persistent knowledge; do not invent a `bn remember` invocation.

**Architecture in one line (kopi-beans):** issues live in git refs — canonical state on `refs/heads/beads/store` (state/deps/tombstones jsonl + meta.json), backups under `refs/beads/backup/*`; a background daemon auto-syncs that ref to your git remote (separate from `refs/heads/*` code branches). Same ref/file layout as beads-rs, which is why the two collide — see the blocker above. `.beads/issues.jsonl` is a local compat-export symlink, not the source of truth. Migrated off beads-rs on 2026-08-07 (itself migrated off Go beads on 2026-07-20).

## Agent Context Profiles

The managed tracker block is task-tracking guidance, not permission to override repository, user, or orchestrator instructions.

- **Conservative (default)**: Use `bn` for task tracking when its store is readable; otherwise use TodoWrite/TaskCreate. Do not run git commits, git pushes, or a manual sync (`bn sync`) unless explicitly asked. At handoff, report changed files, validation, and suggested next commands.
- **Minimal**: Keep tool instruction files as pointers to `bn prime`; use the same conservative git policy unless active instructions say otherwise.
- **Team-maintainer**: Only when the repository explicitly opts in, agents may close issues, run quality gates, commit, and push as part of session close. A current "do not commit" or "do not push" instruction still wins.

## Session Completion

This protocol applies when ending a kopi-beans-tracked implementation workflow. It is subordinate to explicit user, repository, and orchestrator instructions.

1. **File issues for remaining work** - Create tracker issues (or, while the blocker above is open, TodoWrite/TaskCreate items) for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **Handle git/sync by active profile**:
   ```bash
   # Conservative/minimal/default: report status and proposed commands; wait for approval.
   git status

   # Team-maintainer opt-in only, unless current instructions forbid it:
   git pull --rebase
   bn sync            # optional: block until the daemon flush lands
   git push
   git status
   ```
5. **Hand off** - Summarize changes, validation, issue status, and any blocked sync/commit/push step

**Critical rules:**
- Explicit user or orchestrator instructions override this tracker block.
- Do not commit or push without clear authority from the active profile or the current user request.
- If a required sync or push is blocked, stop and report the exact command and error.
<!-- END ISSUE TRACKER INTEGRATION -->

<!-- BEGIN KOPI-BEANS CODEX SETUP (hand-maintained for kopi-beans since 2026-08-07; formerly BEADS CODEX SETUP: generated by bd setup codex) -->
## Issue Tracker (kopi-beans, Codex)

Use kopi-beans (`bn`) for durable task tracking in repositories that include
it. Use the skill at `.agents/skills/beads/SKILL.md` (project install) or
`~/.agents/skills/beads/SKILL.md` (global install) — still named `beads` for
now, but rewritten for `bn` — for workflow guidance, then use the `bn` CLI for
issue operations. **See the blocker note above: `bn` cannot read this repo's
store as of 2026-08-07 ([kopitiam#16](https://github.com/theodoreOnzGit/kopitiam/issues/16)),
so fall back to markdown TODOs until it clears.**

### Quick Reference

```bash
bn ready                # Find available work
bn show <id>            # View issue details
bn claim <id>           # Claim work
bn close <id>           # Complete work
bn prime                # Refresh workflow context
```

### Rules

- Use `bn` for all task tracking once its store is readable; do not create markdown TODO lists otherwise than as the interim fallback.
- Run `bn prime` when tracker context is missing or stale. Codex 0.129.0+ can load tracker context automatically through native hooks; use `/hooks` to inspect or toggle them.
- **`bn` has no `remember` command.** Keep persistent project memory in `MEMORY.md` / per-project memory files, not a `bn remember` invocation that doesn't exist.

**Architecture in one line (kopi-beans):** issues live in git refs — canonical state on `refs/heads/beads/store` (state/deps/tombstones jsonl + meta.json), backups under `refs/beads/backup/*`; a background daemon auto-syncs that ref to your git remote (separate from `refs/heads/*` code branches). Same ref/file layout as beads-rs, which is why the two collide — see the blocker above. `.beads/issues.jsonl` is a local compat-export symlink, not the source of truth. Migrated off beads-rs on 2026-08-07 (itself migrated off Go beads on 2026-07-20).
<!-- END KOPI-BEANS CODEX SETUP -->
