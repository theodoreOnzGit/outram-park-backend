<!-- Moved verbatim from the workspace CLAUDE.md on 2026-09-21 (maintainer direction: physics
     rules stay in the root, everything else here behind a pointer). STILL BINDING. -->

## Agent-fleet progress reporting (HARD RULE, container-timeout prevention)

**Whenever you spawn an agent fleet — any background subagent, parallel agent
wave, or `Workflow` orchestration — you MUST post a summarised progress update
in chat at least every 15 minutes until the fleet is done.** This is a hard
rule, not a courtesy: long silent stretches while agents work let the remote
execution container idle out, and a timed-out container loses the session's
in-flight work.

**What this requires in practice:**

- **Never go quiet waiting on a fleet.** If agents are still running and ~15
  minutes have passed since your last chat message, post an update even when
  there is nothing new to report ("3 of 7 agents still running, no results
  back yet" is a valid update).
- **Summarise, don't dump.** Report what has landed, what is still in flight,
  and anything that failed or needs a decision. Do not paste raw subagent
  transcripts.
- **Schedule the heartbeat, don't rely on remembering it.** Use `send_later`
  (or an equivalent wake-up) at 15-minute intervals when the fleet may outlast
  a single turn, so the update fires even if no agent has reported back.
- **Keep it up until the fleet is fully done**, then post a final summary.
  Stop the heartbeat once there is nothing left running.
- This does **not** relax any other rule — in particular the working-hours
  guardrail above **when the session has opted into it** (with it on, do not
  run fleets outside active hours in the first place) and the
  never-auto-commit/push rule.


## Agents in worktrees, and waiting on processes (2026-10-06)

Learned running a dozen worktree agents in one session (gh:#599 era):

- **Worktrees start from `develop` now.** `.claude/settings.json` sets
  `worktree.baseRef = "head"`, so an `isolation: "worktree"` agent branches
  from the session's own HEAD. Before that it branched from `origin/HEAD`
  (`main`, which has none of the current work) and every agent had to
  `git reset --hard origin/develop` first. A brief may still say "check you
  are on develop"; it is a cheap guard, not a ritual.
- **`reference-data/` resolves to the main checkout from a worktree.** A
  worktree leaves the `reference-data/ace` submodule as an empty directory;
  `njoy_outram_park_fork::reference_data::reference_data_dir` now falls back
  to the main checkout's copy when the worktree's is empty. Setting
  `OUTRAM_PARK_REFERENCE_DATA_DIR` by hand is no longer needed.
- **Never wait with `while pgrep -f "<pattern>"; do sleep …; done`.** The
  loop's own shell command line contains `<pattern>`, so `pgrep -f` always
  matches itself and the loop never ends: one session accumulated ~85 such
  loops, each still "running" hours after the work finished. Wait on a PID
  you recorded (`while kill -0 "$PID" 2>/dev/null; do sleep 30; done`), or
  run the job with `run_in_background` and let the completion notification
  wake you. The same trap applies to `pkill -f` (which is also forbidden by
  name: the maintainer runs their own GUIs).
- **Remove a worktree once its branch is merged** (`git worktree remove`,
  then `git branch -d`), so `.claude/worktrees/` holds only live work.
