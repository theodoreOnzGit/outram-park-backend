# OUTRAM PARK — cost and value estimate

A good-faith estimate, dated **2026-07-13**, of what this workspace cost to
build and what it's worth as an open-source project. Not an audited
appraisal — a reasoned estimate from the repository's own data (LOC, commit
history) plus standard cost-modelling and market-comparable reasoning.
Treat the dollar figures as bands, not point estimates.

## Methodology

- **Size**: counted `.rs` files under each crate's `src/` (and separately,
  including `tests/`/`examples/`/`tutorials/`), excluding `target/` build
  output and gitignored `upstream_source/` vendor clones.
- **Timeline**: read from `git log --format=%ad --date=format:%Y-%m`,
  bucketed by month, across all branches (`git shortlog -sn --all`).
- **Cost**: two lenses — a traditional COCOMO-organic estimate (effort ≈
  `2.4 × KLOC^1.05` person-months, the standard formula for a small,
  domain-familiar team with no unusual process overhead) as an upper-bound
  anchor, versus a grounded reconstruction-cost estimate based on what the
  git history shows actually happened.
- **Value**: cost is not the same question as value for an unmonetized
  GPL-3.0 project — framed separately via replacement/utility value,
  strategic/portfolio value, and educational value.

## Results

### Size

| Metric | Count |
|---|---|
| Crates | 13 |
| `.rs` files (all, excl. `target/`, `upstream_source/`) | 1,588 |
| Lines in `src/` only | ~337,000 |
| Lines total (incl. `tests/`, `examples/`, `tutorials/`) | ~388,000 |

Largest crates by `src/` line count: `tuas_boussinesq_solver` (~161K —
mostly CIET facility pre-built components and their regression/calibration
datasets, not a vendored copy of anything), `tampines-steam-tables` (~77K,
including a ~16K-line vendored `openfoam_algorithms` copy),
`outram-park-fork-coolprop` (~32K), `njoy-outram-park-fork` (~21K).

### Timeline

```
2024-11:   1 commit
2026-06: 123 commits
2026-07: 134 commits (through 2026-07-13)
```

Contributors (`git shortlog -sn --all`): `teddy0@snrsiDesktop` (165),
`teddy0@arch-desktop-aftershock` (98), `Claude` (3), `teddy0_work` (2),
`theodoreOnzGit` (1) — i.e. one person across two machines, with the
overwhelming majority of commit *volume* landing in a roughly six-to-seven
week window (June-July 2026), not spread evenly across the ~19.5 months
since the first commit. The single Nov 2024 commit reads as an initial
scaffold, not the start of sustained work.

## Cost

**Naive traditional-methodology estimate (COCOMO-organic, no AI, small
team, novel-design assumptions):** for 337 KLOC, effort ≈ 80-100
person-years. **Not treated as credible for this project** — it assumes
from-scratch design productivity, and most of this codebase is direct
translation of existing reference implementations (OpenFOAM, OpenMC, NJOY,
CoolProp, rust-steam), which is inherently faster than invention. Kept here
only as an upper-bound anchor.

**What actually happened:** one domain expert, AI-paired, over roughly six
to seven weeks of intense work (matching the commit-date concentration
above). The *actual* cost incurred is therefore small in dollar terms — the
maintainer's own time plus AI tooling/compute cost, likely low four figures
in subscription/compute spend.

**Replacement cost** (what a third party would have to pay to commission
equivalent work today, hiring a comparable specialist plus AI tooling):
estimated **$200K-$600K**, reflecting roughly 1-3 FTE-years of a rare skill
combination — nuclear-engineering domain literacy plus Rust/numerical
systems engineering — at a fully-loaded $150-220K/year rate. Without AI
assistance, several times higher — plausibly $1-3M — which is the gap AI
leverage is buying here.

## Value

As GPL-3.0 open-source software with no monetization path, there is no
meaningful "market value" in a sale sense — nobody buys this outright, and
there is no visible external contributor base or install footprint yet (one
external collaborator, "Ethan," appears in the beads history). Value reads
better along three separate axes:

- **Replacement/utility value** to whoever would otherwise need this
  capability (a research group, a small reactor-safety consultancy, or a
  nuclear-engineering education program) — roughly the same $200-600K band
  as the reconstruction-cost estimate above, since that is what they would
  pay to commission it.
- **Strategic/portfolio value** to the maintainer — a working, validated,
  cross-domain simulation suite (neutronics, Monte Carlo transport, CFD,
  steam properties, thermal-hydraulics, process control) is a strong
  research/consulting credibility asset independent of any sale price.
- **Educational value** — the crate docs are explicitly designed for
  readability and video-tutorial-friendly explanation (the `docs/`
  convention: theory → numerics → implementation → walkthrough), and the
  FHR and CIET educational simulators exist specifically for teaching —
  real value to training programs even with zero commercial licensing.

## Is the work easily replaced?

Split answer — this is the honest core of it.

**The mechanical translation work: yes, fairly easily**, especially now.
This week's own session is the evidence: a full four-package crate rename
touching 564 files, a rustdoc-to-markdown documentation pipeline, hundreds
of ported fluid/mixture records — all inside single sessions. Any
comparably domain-literate engineer with similar AI fluency could reproduce
the *translated* parts of this codebase in a similar timeframe.

**What's genuinely hard to replace fast is the validated, coherent whole.**
Three specific things:

1. **Domain judgment to catch subtle physics bugs.** Real examples from
   this project's own history: the RECONR grid-density bug behind the
   U-238 capture "wing" artifact in the NJOY port, and the near-bubble-point
   HEM artifact in the choked-flow solver. Both took real debugging
   judgment to isolate — not just code generation.
2. **The accumulated verification-against-reference-data record.** Moody /
   Zaloudek / Marviken critical-flow benchmarks, IAPWS-IF97 steam-table
   verification, CIET facility regression data. Rebuilding trust in a
   physics codebase takes calendar time and domain literacy regardless of
   how fast code itself can be generated.
3. **Uniform architectural discipline across 13 crates** — enum dispatch
   (no trait objects), `uom`-typed units throughout, a hard per-file
   line-count cap, a documented human-interface-layer mandate. A fresh team
   assembling comparable breadth without the same enforced rules would
   likely end up more fragmented.

**Bottom line:** the code is replaceable; the *validated* code with a
domain expert's judgment behind it is the actual scarce asset — and that
scarcity is a function of the maintainer's expertise and AI-augmented
workflow, not raw labor-hours invested.
