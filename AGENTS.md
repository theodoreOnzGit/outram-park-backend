# Agent Instructions

This project tracks work in **GitHub issues** on
`theodoreOnzGit/outram-park-backend`, using the `gh` CLI. The workspace rules
are in [`CLAUDE.md`](CLAUDE.md) and they bind every agent, not only Claude.
This file is a short pointer for agents that read `AGENTS.md` first.

> **Tracker history.** kopi-beans (`bn`) was deprecated on 2026-09-21 because
> of persistent problems with its git-ref store and sync daemon. Before that
> the workspace used beads-rs (`bd`) and, before that, Go beads. Do not install
> or use `bn` or `bd` here. The full record is in
> [`docs/kopi-beans-deprecation.md`](docs/kopi-beans-deprecation.md).

## Quick Reference

```bash
gh issue list --state open                       # what is open
gh issue view <number>                           # details + comments
gh issue create --title "..." --body "..."       # file one
gh issue comment <number> --body "..."           # progress
```

## Rules

- Use `gh` issues for ALL task and roadmap tracking. Do not use TodoWrite,
  TaskCreate or markdown TODO lists.
- **Do not close issues on your own initiative.** Propose the closure with its
  evidence; the maintainer decides.
- **Labels:** `bug`, `enhancement`, `epic`, `P0`…`P3`. There is one epic per
  member crate. GitHub has no dependency graph, so state blocking relationships
  in the issue body in words ("blocked by #123").
- **After a plan is approved, convert it into issues before writing code**:
  one child issue per deliverable, under the relevant crate's epic.
- **`op-*` ids are historical** references into the old beads tracker. Do not
  look them up in `gh` and do not mint new ones.
- **If `gh` is unavailable**, fall back to the harness's task tools and say so
  in the hand-off.
- The tracker holds *work to do*. Durable facts and preferences go in the
  per-project `memory/` files and `MEMORY.md`.

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

## Session Completion

This protocol applies when ending an implementation workflow. It is
subordinate to explicit user, repository, and orchestrator instructions.

1. **File issues for remaining work** with `gh issue create`.
2. **Run quality gates** (if code changed): tests, linters and builds, all in
   release mode (`--release`). Say which suite you ran.
3. **Update issue status**: comment progress on in-progress issues and propose
   closures rather than making them.
4. **Handle git conservatively**: report `git status` and the proposed
   commands, and wait for approval. Do not commit or push unless the user or
   the maintainer's configured stop hook asks for it, and never push to `main`
   without an explicit request.
5. **Hand off**: summarise changes, validation, issue status, and any blocked
   commit/push step with the exact command and error.
