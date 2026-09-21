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

