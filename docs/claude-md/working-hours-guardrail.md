<!-- Moved verbatim from the workspace CLAUDE.md on 2026-09-21 (maintainer direction: physics
     rules stay in the root, everything else here behind a pointer). STILL BINDING. -->

## Working-hours guardrail (OPT-IN — off unless the user turns it on)

**This guardrail is OFF by default, and nothing about it is mandatory** —
there is no required question to ask at session start and no required time
check to run. It is not a standing rule. Changed 2026-08-13, and further
relaxed 2026-09-03, at the maintainer's request.

**Turning it on.** The guardrail applies only when the user turns it on in
plain words ("enable the working-hours guardrail", "enforce my hours this
session"). You are **not** required to ask about it — offer it only if the
user seems to want it, and **treat silence as Off, always.** The user may
switch it on or off at any point; honour that immediately, with no
confirmation question.

**If it is off (the default):** no time check, no hour restriction, no
rest-day rule. Work normally. Do not volunteer reminders about the
maintainer's hours or health, and do not re-litigate the setting.

**If the user has turned it on**, everything below applies for the rest of
that session, as a hard rule.

**Check the real local time and day of week** with a system tool before
substantive work — do not infer it from conversation content, a cached date,
or skip the check. Preferred: `date +'%Y-%m-%d %H:%M %A %Z'`.

**Active working hours** (local to the repository owner, Asia/Singapore):

| Day | Hours |
|---|---|
| Monday – Friday | 07:30 – 20:00 |
| Sunday | 12:00 – 19:00 |
| Saturday | none — full rest day |

**Outside these hours, with the guardrail enabled:**

- Do **not** answer substantive questions or add context, analysis, or
  explanation beyond the minimum needed to log something for later.
- Do **not** agentically write code, run test suites, or open-endedly work a
  task.
- Ideas, plans, or scaffolding may be recorded — as a GitHub issue or a short
  markdown note — and nothing more.
- **Exception, still allowed:** compiling / running the existing test suite to
  confirm already-finished work is good, and pushing already-finished work to
  GitHub. Nothing beyond finishing and shipping work that already exists.

**While enabled, the hour limits do not bend in the moment.** Turning the
guardrail on is a deliberate decision; asking for a one-off exception at 23:00
is not. If the user asks to work past the limit *within an enabled session*,
say so plainly, log the request for the next active window, and stop there —
do not negotiate or justify. Turning the guardrail off outright is always the
user's call and is honoured immediately; what this clause blocks is piecemeal
erosion while it is on.

**Why it exists.** It protects the human maintainer's rest. Instituted
2026-07-11 after a month of illness from overwork; the supporting analysis is
in [`DEVELOPER_HEALTH_WARNING.md`](../../DEVELOPER_HEALTH_WARNING.md). Making it
opt-in does not retract that finding — it moves the decision to the human.
**To restore it as an always-on rule**, edit this section accordingly. That is
a maintainer decision.

