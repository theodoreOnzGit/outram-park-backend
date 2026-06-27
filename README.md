# outram-park-backend
Backend for Open-source TRAnsient Multi-Phase Advanced Reactor simulator Kit (OUTRAM PARK), a suite for real-time simulators

## Note on AI-assisted development cost vs. value

Much of the recent work on this workspace — the dependency / egui 0.34 migration,
the `peroxide` adaptive-integration hang fix in TUAS, and the TAMPINES
near-saturation (x ≈ 0) choked-flow fix — was done with AI assistance (Claude
Code, a mix of Sonnet 4.6 and Opus 4.8). This note records, for future
reference, whether that spend was worthwhile.

### Cost

Token usage was not metered precisely here; estimating it from git history alone
is unreliable, because Claude Code's cost is dominated by *input* (re-sent
context, file contents, tool/test output), not by the committed diff. Model
prices for reference (per 1M tokens):

| Model | Input | Output | Cache write (5m / 1h) | Cache read |
|---|---|---|---|---|
| Opus 4.8 | $5.00 | $25.00 | $6.25 / $10.00 | $0.50 |
| Sonnet 4.6 | $3.00 | $15.00 | $3.75 / $6.00 | $0.30 |

Cache reads (0.1× input) dominate in practice because Claude Code caches
aggressively, so a naive "all input × $5" overestimates by 3–10×. **Assume a
conservative upper bound of ~$600** for the work above.

### Comparison to a human worker

The work needs a rare skill combination: production Rust **and** numerical
methods **and** nuclear/chemical-engineering thermodynamics (IAPWS-IF97,
two-phase critical flow, HEM, sound-speed behaviour at the bubble point) **and**
an egui GUI migration. A specialist who can do all of this bills roughly
**$120–250/hr** as a contractor (≈ $90–150/hr fully loaded as an employee). The
output produced is plausibly **3–10 focused engineer-days** for such a person,
i.e. **$2,400–$12,000**.

So at a conservative $600, the AI-assisted cost is roughly **5–25% of the
human-equivalent cost** for the same output, and compresses the calendar time
from weeks to days.

### The honest caveat

This is **augmentation, not replacement** — the value is *conditional on expert
review*. The AI did not work autonomously: a domain expert set the direction,
made the key calls (e.g. the interpolation-vs-sonic-fix decision in the choked-
flow work), supplied domain notes, and verified every result. The AI also
thrashed on the hardest problem (the x ≈ 0 fix took several iterations and
introduced regressions that had to be caught and undone). An unreviewed AI fix
to a thermodynamics solver that is *subtly* wrong could cost far more than $600
downstream (bad simulation output, a wrong published number).

**Verdict:** worthwhile for this mix of work — bounded-tedious migration (ideal
AI territory) and exploratory debugging *with a checkable oracle* (the Zaloudek
validation tests). It is **not** a safe substitute on work with no verification
oracle, or if AI output is treated as correct-by-default. The $600 buys
specialist-grade output at a fraction of specialist cost **provided** the expert
stays in the loop to verify it; the review is what makes the spend safe rather
than risky.
