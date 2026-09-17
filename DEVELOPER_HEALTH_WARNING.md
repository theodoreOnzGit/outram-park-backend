# DEVELOPER HEALTH WARNING: AGENTIC SOFTWARE DEVELOPMENT

> **WARNING:** Outram Park was developed using highly agentic software engineering workflows involving Claude Code, Microsoft Copilot, ChatGPT, Gemini, and other AI-assisted development tools. While these tools can dramatically increase engineering productivity, they may also increase the risk of technostress, cognitive fatigue, decision fatigue, hyperfocus, insomnia, burnout, and prolonged illness.
>
> Developers are encouraged to treat wellbeing, recovery, and sustainable working practices as first-class engineering requirements.

# Why This Document Exists

This document was written after approximately one month of intensive agentic software development on the Outram Park project.

During this period, the maintainer experienced:

* Persistent fatigue.
* Decision fatigue.
* Difficulty disengaging from development activities.
* Hyperfocus lasting many consecutive hours.
* Sleep disruption and insomnia.
* Reduced awareness of accumulating exhaustion.
* A prolonged sore throat lasting approximately one month.
* Continued development activity despite recognized fatigue.

These experiences motivated a review of:

* Repository commit history.
* Working-hour patterns.
* Existing literature on technostress.
* Existing literature on cognitive fatigue.
* Existing literature on hyperfocus.
* Existing literature on psychological stress and immune function.

The goal of this document is to help future contributors avoid repeating the same mistakes.

# 1. Anecdotal Evidence from the Outram Park Project

## Commit History Analysis

A review of repository history identified:

| Metric                           | Value |
| --------------------------------- | ----: |
| Total commits analysed           |   282 |
| Commits inside working hours     |   201 |
| Commits outside working hours    |    81 |
| Percentage outside working hours | 28.7% |

This means:

* Nearly **29% of all commits** occurred outside working hours.
* Roughly **1 in every 3.5 commits** was made outside the eventual safety window.
* Almost one-third of repository activity occurred during periods later designated as recovery or off-work time.

### Breakdown of Outside-Hours Activity

| Category                                    |  Count |
| -------------------------------------------- | -----: |
| Weekday commits before 07:30 or after 20:00 |     52 |
| Sunday commits outside permitted hours      |     13 |
| Saturday commits (full rest day)            |     16 |
| **Total outside-hours commits**             | **81** |

Observed patterns included:

* Late-night development sessions.
* Activity extending beyond 22:00.
* Occasional development after midnight.
* Multiple weekend coding sessions.
* Repeated Sunday evening work.
* Several full Saturdays spent developing software.

Importantly, the working-hours policy discussed later did **not** exist during most of this period. The majority of these commits therefore do not represent policy violations; rather, they represent historical behaviour that preceded the creation of project-level health safeguards.

Nevertheless, the commit history provides an objective record of sustained after-hours development activity.

## The Productivity Trap

One particularly important observation was that excessive work was not primarily driven by project deadlines.

A major motivating factor was:

> Maximizing utilization of a Claude Pro subscription.

The thought process often looked like:

```text
Available token budget

down arrow

Unused AI capacity

down arrow

"I should use what I am paying for"

down arrow

"One more feature"

down arrow

"One more benchmark"

down arrow

"One more refactor"

down arrow

Several additional hours of work
```

This behaviour was reinforced by:

* Immediate implementation availability.
* Extremely short development feedback loops.
* Rapid visible project progress.
* The feeling that unused model capacity represented wasted value.

In retrospect, this created a novel productivity trap:

> Because software development had become unusually inexpensive, there was increased pressure to continue generating more work.

The AI model never became tired.

The developer did.

## The One-Month Illness

During the same period, the maintainer experienced:

* Persistent fatigue.
* Sleep disruption.
* Decision fatigue.
* A sore throat lasting approximately one month.

This document does **not** claim that AI tools directly caused illness.

However, these observations motivated a review of literature exploring the relationship between:

* Technostress,
* Hyperfocus,
* Sleep disruption,
* Cognitive fatigue,
* Psychological stress,
* Immune function.

# 2. Engineering Safety Controls

## Working-Hour Restrictions

Following this period, formal safeguards were introduced into `CLAUDE.md`.

Agentic development is restricted to:

| Day           | Working Hours          |
| -------------- | ----------------------- |
| Monday-Friday | 07:30-20:00            |
| Saturday      | No agentic development |
| Sunday        | 12:00-19:00            |

Outside these periods:

* Claude Code must not perform agentic development work.
* Claude Code may assist only with planning activities.
* Tasks should instead be recorded using Beads or TODO systems.

### Amendment, 2026-08-13 — the restriction is now opt-in

At the maintainer's request (relayed from a colleague), the `CLAUDE.md`
guardrail was changed from an always-on hard rule to an **opt-in setting the
assistant asks about once at the start of each session**, defaulting to **off**.
The hours in the table above are unchanged and still apply in full whenever a
session opts in; what changed is that the assistant no longer enforces them
unilaterally.

The findings in section 1 of this document are **not** retracted by that
amendment. Adherence is now a matter of maintainer discipline rather than
tooling, which removes the automated backstop the original policy provided.

### Amendment, 2026-09-03 — the per-session question is no longer mandatory

At the maintainer's request, the mandatory "ask once at the start of each
session" step was dropped. The guardrail is still available and still defaults
to **off**; the difference is that the assistant no longer has to ask about it
and only applies it when the user turns it on in plain words. The hours in the
table above are unchanged and still apply in full whenever the user enables it.
The section-1 findings are, again, not retracted.

## Purpose of the Policy

The purpose of this policy is not to reduce productivity.

The purpose is to:

* Protect sleep.
* Protect weekends.
* Protect long-term sustainability.
* Reduce burnout risk.
* Preserve engineering judgement.
* Prevent recurrence of prolonged illness.

# 3. Literature Review

## Technostress

Technostress refers to stress arising from technology use and technology-enabled work.

Tarafdar, Tu, and Ragu-Nathan (2007) introduced several major dimensions of technostress:

* Techno-overload.
* Techno-invasion.
* Techno-complexity.
* Techno-insecurity.
* Techno-uncertainty.

(Tarafdar et al., 2007)

### Techno-Overload

Techno-overload occurs when technology increases the amount of work an individual can perform and therefore increases expectations and workload (Tarafdar et al., 2007).

Agentic development workflows may amplify techno-overload because:

* Code generation becomes dramatically faster.
* Refactoring becomes inexpensive.
* Architectural experimentation becomes easier.

The bottleneck therefore shifts from implementation toward:

* Review,
* Verification,
* Validation,
* Decision making.

### Techno-Invasion

Techno-invasion occurs when technology erodes the boundary between work and personal life (Tarafdar et al., 2007).

Examples include:

* Evening coding sessions.
* Weekend work.
* Difficulty disengaging from projects.
* The feeling that productive work remains continuously available.

Tarafdar, Cooper, and Stich (2019) note that technology can simultaneously generate benefits ("techno-eustress") and harms ("techno-distress"), highlighting the need for deliberate safeguards (Tarafdar et al., 2019).

## Hyperfocus

Hyperfocus refers to periods of unusually intense attentional engagement.

Research has documented hyperfocus across autistic, ADHD, and general populations (Dwyer et al., 2024).

Dwyer et al. (2024) reported associations between hyperfocus and:

* Reduced quality of life.
* Repetitive thinking.
* Greater anxiety symptoms.
* Attentional regulation difficulties.

At the same time, hyperfocus is not inherently negative.

Dupuis et al. (2022) found that intense attentional engagement may also function as a genuine cognitive strength that contributes to productivity and deep work.

Consequently:

> Hyperfocus may facilitate exceptional progress while simultaneously increasing the risk of overwork.

Warning signs include:

* Skipping meals.
* Missing breaks.
* Delaying sleep.
* Losing awareness of time.
* Continuing work despite obvious fatigue.

(Dwyer et al., 2024; Dupuis et al., 2022)

## Cognitive Fatigue

Cognitive fatigue emerges following prolonged periods of demanding mental effort.

Pessiglione et al. (2025) reviewed evidence suggesting that cognitive fatigue:

* Accumulates over time.
* Is often poorly recognized by the individual experiencing it.
* Alters subsequent decision making.
* Reduces willingness to engage in demanding cognitive tasks.

(Pessiglione et al., 2025)

Steward et al. (2025) further demonstrated that repeated cognitive exertion alters subsequent effort-based decision making.

Their findings suggest that fatigue affects not only performance but also future willingness to exert effort (Steward et al., 2025).

In agentic development:

* Coding effort decreases.
* Review effort increases.
* Validation effort increases.
* Architectural decision making increases.

Consequently, significant cognitive fatigue may develop despite reduced manual programming effort.

## Decision Fatigue

Decision fatigue refers to deterioration in decision quality following prolonged decision making.

Choudhury and Saravanan (2026) identified several consequences:

* Reduced efficiency.
* Lower-quality decisions.
* Increased cognitive burden.
* Choice avoidance.

(Choudhury & Saravanan, 2026)

Agentic systems frequently generate:

* Multiple architectures.
* Multiple valid implementations.
* Multiple debugging approaches.
* Multiple optimization strategies.

The developer therefore becomes responsible for choosing among many plausible alternatives.

AI systems may therefore reduce implementation effort while simultaneously increasing decision-making effort.

## Stress, Immunity, and Illness

The relationship between psychological stress and immune function is one of the strongest empirical foundations relevant to this document.

Cohen, Tyrrell, and Smith (1991) conducted a landmark viral challenge study in which healthy volunteers were intentionally exposed to respiratory viruses.

They observed a dose-dependent relationship between psychological stress and susceptibility to respiratory illness (Cohen et al., 1991).

Participants experiencing higher levels of psychological stress were more likely to:

* Become infected.
* Develop clinical cold symptoms.

(Cohen et al., 1991)

Subsequent reviews concluded that psychological stress is associated with measurable changes in immune functioning and increased susceptibility to upper respiratory infections (Cohen, 1995; Cohen, 1996).

The literature therefore supports the plausibility of the following pathway:

```text
Agentic development
-> Technostress
-> Hyperfocus
-> Extended working hours
-> Sleep disruption
-> Chronic psychological stress
-> Immune system effects
-> Increased susceptibility to illness
```

Importantly, the literature does **not** demonstrate that AI tools directly cause illness.

Instead, it demonstrates that prolonged stress, fatigue, poor recovery, and sleep disruption may contribute to physiological states associated with increased illness susceptibility (Cohen et al., 1991; Cohen, 1995; Cohen, 1996).

## Key Takeaway

The current scientific literature does not yet contain extensive studies specifically examining long-duration agentic coding workflows.

However, converging evidence from:

* Technostress research (Tarafdar et al., 2007; Tarafdar et al., 2019),
* Hyperfocus research (Dwyer et al., 2024; Dupuis et al., 2022),
* Cognitive fatigue research (Pessiglione et al., 2025; Steward et al., 2025),
* Decision fatigue research (Choudhury & Saravanan, 2026), and
* Stress-immunity research (Cohen et al., 1991; Cohen, 1995; Cohen, 1996)

suggests that highly productive AI-assisted development workflows should be treated as potential occupational health risks when recovery, boundaries, and sustainable work practices are neglected.

# 4. Recommendations

## Protect Sleep

Sleep is an engineering dependency.

Treat insufficient sleep as seriously as:

* Failing tests.
* Broken builds.
* Invalid validation results.

## Record Decisions Instead of Continuing Work

When fatigued:

* Write notes.
* Create TODO items.
* Create Beads tasks.

Do not force major architectural decisions.

## Respect Stopping Points

Establish explicit end-of-day criteria.

Do not rely on:

> "Just one more task."

## Preserve Recovery Time

Protect:

* Sleep.
* Meals.
* Exercise.
* Weekends.
* Annual leave.
* Family time.

## Remember the Human Bottleneck

> AI systems do not become tired.
>
> Developers do.

A highly productive project that burns out its developers has failed one of its most important engineering constraints.

# References

*(APA 7th edition)*

Choudhury, N. A., & Saravanan, P. (2026). An integrative review on unveiling the causes and effects of decision fatigue to develop a multi-domain conceptual framework. *Frontiers in Cognition*, *4*, Article 1719312. <https://doi.org/10.3389/fcogn.2025.1719312>

Cohen, S. (1995). Psychological stress and susceptibility to upper respiratory infections. *American Journal of Respiratory and Critical Care Medicine*, *152*(4 Pt 2), S53-S58. <https://doi.org/10.1164/ajrccm/152.4_Pt_2.S53>

Cohen, S. (1996). Psychological stress, immunity, and upper respiratory infections. *Current Directions in Psychological Science*, *5*(3), 86-89. <https://doi.org/10.1111/1467-8721.ep10772808>

Cohen, S., Tyrrell, D. A. J., & Smith, A. P. (1991). Psychological stress and susceptibility to the common cold. *New England Journal of Medicine*, *325*(9), 606-612. <https://doi.org/10.1056/NEJM199108293250903>

Dupuis, A., Mudiyanselage, P., Burton, C. L., Arnold, P. D., Crosbie, J., & Schachar, R. J. (2022). Hyperfocus or flow? Attentional strengths in autism spectrum disorder. *Frontiers in Psychiatry*, *13*, Article 886692. <https://doi.org/10.3389/fpsyt.2022.886692>

Dwyer, P., Williams, Z. J., Lawson, W. B., & Rivera, S. M. (2024). A trans-diagnostic investigation of attention, hyper-focus, and monotropism in autism, ADHD, and the general population. *Neurodiversity*, *2*. <https://doi.org/10.1177/27546330241237883>

Pessiglione, M., Blain, B., Wiehler, A., & Naik, S. (2025). Origins and consequences of cognitive fatigue. *Trends in Cognitive Sciences*, *29*(8), 730-749. <https://doi.org/10.1016/j.tics.2025.02.005>

Steward, G., Looi, V., & Chib, V. S. (2025). The neurobiology of cognitive fatigue and its influence on effort-based choice. *Journal of Neuroscience*, *45*(24), Article e1612242025. <https://doi.org/10.1523/JNEUROSCI.1612-24.2025>

Tarafdar, M., Cooper, C. L., & Stich, J.-F. (2019). The technostress trifecta-techno-eustress, techno-distress and design: Theoretical directions and an agenda for research. *Information Systems Journal*, *29*(1), 6-42. <https://doi.org/10.1111/isj.12169>

Tarafdar, M., Tu, Q., & Ragu-Nathan, T. S. (2007). The impact of technostress on role stress and productivity. *Journal of Management Information Systems*, *24*(1), 301-328. <https://doi.org/10.2753/MIS0742-1222240109>
