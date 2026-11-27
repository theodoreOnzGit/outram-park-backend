# outram-park-backend

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


Cargo workspace for **OUTRAM PARK** — Open-source Unified TRAnsient Multi-Physics Advanced Reactor simulation Kit.

~~A suite of Rust libraries for real-time thermal-hydraulics, reactor kinetics, steam-cycle thermodynamics, and compressible CFD simulation.~~ **CHANGED 2026-11-27 (maintainer: "it is not the simulations themselves, it is knowledge preservation and presentation"):** an open-source ecosystem that turns scientific and technical knowledge (nuclear data, Monte Carlo and deterministic transport, thermal-hydraulics, CFD, fuel and materials behaviour, dispersion and dose, and the literature behind them) into **inspectable source code, interactive simulations, visualisations and verification evidence** that people can study and challenge. It is 51 Rust crates, most of them translations of established open-source codes whose every routine can be read beside its upstream. The simulations are a means; the preserved, connected and challengeable knowledge is the point (see the vision below).

**Code deep dives and API reference:** <https://theodoreonzgit.github.io/outram-park-backend/>
(how each crate is built and why, beside its code; rebuilt from `develop` when it is merged into `web-deploy`
by `.github/workflows/pages.yml`, gh:#518; ~~on every push to `develop`~~ **CHANGED 2026-11-06**, gh:#1306). The concept lessons for
students will live on the [OUTRAM PARK teaching site](https://theodoreonzgit.github.io/outram-park/) (not live yet; outram-park#5).

> *"Best open-source nuclear simulator suite in Singapore, JB — and some say Batam!"* 🇸🇬
> — with apologies to **Phua Chu Kang**. Said in fun, lah. For the real, sober
> status, see the ⚠️ banner above: everything here is unverified until validated,
> and **not** for facility operation.

## How it started

It started with curiosity, and with simulators. The author had a keen interest
in nuclear engineering and in software engineering (Rust especially), and a
wider curiosity about many areas of science. Out of that came a simple wish:
wouldn't it be nice if knowledge were freely accessible, for the betterment of
mankind and for the fun of learning?

OUTRAM PARK is where those threads tie together, now that agentic AI is a
reality. Any simulation can be constructed; any knowledge can be visualised
effectively. OUTRAM PARK is a platform for learning and research in anything
related to nuclear science and engineering. The simulators are how the
knowledge is made tangible; the knowledge, kept open to study and challenge,
is the point (see the [Vision](#vision-scientific-knowledge-that-can-be-studied-and-challenged)).

The first simulator started as a sketch. An early concept from 2025, drawn by hand on graph
paper, titled **"Natural FHR Simulator, Mark I"**: a fluoride-salt-cooled
high-temperature reactor (FHR) you could watch and operate.

![Hand-drawn 2025 concept sketch of the Natural FHR Simulator Mark I: a pebble-bed core with control rod, graphite reflector and decay-heat removal loop; a primary salt pump labelled TUAS; an intermediate salt-to-salt heat exchanger; a helical steam generator, single-stage expansion turbine and condenser labelled TAMPINES](docs/images/fhr-simulator-mark-i-concept-2025.jpg)

Everything on that page was a question for code to answer. On the left, the
core: a pebble bed with a temperature-sensitive display, a control rod, a
graphite reflector and a decay-heat removal loop. In the middle, a primary
salt pump driving the salt through an intermediate salt-to-salt heat
exchanger. On the right, a helical steam generator feeding a single-stage
expansion turbine and a condenser. The sketch is honest about what it left
out: "no HP, LP turbine for simplicity", and a primary pump that "really has
parallel but simplify".

Two labels on it already name the pieces that would carry the idea: **TUAS**
under the salt loops, and **TAMPINES** under the steam side. Both are crates in
[`outram-park-backend`](https://github.com/theodoreOnzGit/outram-park-backend)
today (`tuas_boussinesq_solver` and `tampines`), and the rest of OUTRAM PARK
grew around them.

## Why the names: Singapore's MRT

Nuclear science codes often carry names chosen for their familiarity. At Idaho
National Laboratory many are named after animals: the MOOSE framework (the
Multiphysics Object-Oriented Simulation Environment), BISON for fuel
performance, Griffin for reactor physics, Pronghorn for coarse-mesh
thermal-hydraulics.[^inl] Some are acronyms that read like people's names:
STACY, the Source Term Analysis Code System for fission-product release from
HTR fuel,[^stacy] and SAM, Argonne's System Analysis Module for advanced
non-light-water reactors.[^sam] Some are food: SCONE, Cambridge's Stochastic
Calculator Of Neutron transport Equation.[^scone]

The author wanted names that were familiar and uniquely Singaporean. The first
ideas were food and drink: Cheng Teng, Teh O, Teh C (one survives, in
[`teh-o-prke`](crates/teh-o-prke)). After some discussion with his supervisor,
he agreed that a naming system for codes should be a little more professional.
So came **TUAS**,[^tuas] and with it the convention: name the codes after
places in Singapore, so that people would know they were made in Singapore.
(And, jokingly, as the best open-source nuclear science simulators in
Singapore, JB, and some say Batam.)

Thereafter the codes were named after MRT stations and other places in
Singapore. The convention carries a wish: that learning about nuclear science
should be as accessible and affordable as taking the MRT, and that the fear
and mystery around nuclear science will fade and give way to knowledge. The
convention spread beyond the author's own codes: his colleague Than Yan Ren
named his reactor simulator **BEDOK**,[^bedok] now translated here as
[`bedok`](crates/bedok). The rest was history. Today each station names a
domain of the ecosystem ([`docs/ecosystem-naming.md`](docs/ecosystem-naming.md)).

[^inl]: Idaho National Laboratory, NCRC code descriptions,
    <https://inl.gov/ncrc/code-descriptions/>; MOOSE applications list,
    <https://mooseframework.inl.gov/help/inl/applications.html>. No source was
    found stating why INL chose animal names; the pattern is observed, not a
    documented policy.
[^stacy]: STACY (Source Term Analysis Code System), OECD NEA Data Bank
    package IAEA1439, <https://oecd-nea.org/tools/abstract/detail/iaea1439/>;
    RSICC abstract, <https://rsicc-docs.ornl.gov/abstract/S/STACY--00.html>.
[^sam]: R. Hu et al., "SAM: A Modern System Code for Advanced Non-LWR Safety
    Analysis", *Nuclear Technology* 211(9) (2025),
    <https://www.ornl.gov/publication/sam-modern-system-code-advanced-non-lwr-safety-analysis>;
    Argonne National Laboratory, System Analysis Module,
    <https://www.anl.gov/nse/system-analysis-module>.
[^scone]: SCONE (Stochastic Calculator Of Neutron transport Equation),
    University of Cambridge, MIT licence, <https://github.com/CambridgeNuclear/SCONE>;
    documentation <https://scone.readthedocs.io/en/latest/>.
[^tuas]: T. K. C. Ong, S. Xiao, P. F. Peterson, "An open-source Thermo-hydraulic
    Uniphase Advection and Convection Solver for Salt Flows (TUAS)",
    *International Journal of Advanced Nuclear Reactor Design and Technology*
    (2025), <https://doi.org/10.1016/j.jandt.2025.03.006>.
[^bedok]: Y. R. Than, S. Xiao, "BEDOK: An In-House Numerical Reactor Simulator
    Effort in Singapore", *Energy Engineering* 123(9) (2026),
    <https://doi.org/10.32604/ee.2026.082487>.

## Vision: Scientific Knowledge That Can Be Studied and Challenged

OUTRAM PARK is an open-source ecosystem dedicated to making vast amounts of scientific and technical knowledge **connectable, accessible, understandable, and explorable**.

Generative AI is increasingly capable of summarising, organising, and presenting scientific information. OUTRAM PARK extends these capabilities by transforming scientific knowledge into **executable software, interactive simulations, visualisations, and inspectable computational artifacts**.

The objective is not merely to generate scientific software, but to make the underlying science more accessible to human understanding, examination, and critical inquiry.

### Scientific Software as a Knowledge Artifact

Agentically generated scientific software is not treated as an authority to be trusted by default.

Instead, the generated source code is itself a knowledge artifact that humans can **study and challenge**.

Researchers, students, and engineers can examine implementations to understand how mathematical equations, numerical methods, physical models, and scientific assumptions are translated into computational procedures.

They can challenge these implementations by questioning their assumptions, examining their algorithms, reproducing calculations, identifying errors, and comparing their behaviour against analytical solutions, established software, and experimental evidence.

This process serves two complementary purposes:

1. **Human capability development:** The examination and verification of scientific software provide opportunities to develop expertise in scientific computing, numerical methods, physics, and engineering.
2. **Scientific software development:** Human review, verification, and validation progressively establish evidence for the correctness and applicability of the generated implementations.

The software is therefore both an object of scientific investigation and a potential product of that investigation.

### Visualising Scientific Phenomena

OUTRAM PARK also aims to make scientific phenomena accessible through interactive simulations and visualisations.

Scientific literature frequently describes physical behaviour through mathematical equations, numerical models, and experimental results. While these representations are essential, they can be difficult to interpret without specialised expertise.

By implementing published mathematical models as interactive simulations, OUTRAM PARK allows users to explore physical phenomena, investigate parameter sensitivities, and develop intuition about the underlying science.

These simulations can support:

- Scientific literature review and exploration.
- Nuclear science and engineering education.
- Interactive presentations and public outreach.
- Understanding of mathematical and numerical models.
- Investigation of physical assumptions and model limitations.

Educational simulations can provide value even before their implementations have completed verification and validation, provided their limitations are clearly communicated.

However, **educational usefulness must not be confused with predictive engineering accuracy**. Unverified implementations may contain numerical or physical errors, and their behaviour must not be treated as authoritative without appropriate supporting evidence.

### Connecting Scientific Knowledge

OUTRAM PARK brings together multiple representations of scientific knowledge:

| Knowledge artifact | Purpose |
|---|---|
| Scientific literature | Documents theories, mathematical models, experiments, and published findings. |
| Mathematical equations | Expresses the governing relationships underlying scientific phenomena. |
| Source code | Provides inspectable computational implementations of scientific models. |
| Interactive simulations | Allows users to explore model behaviour and physical phenomena. |
| Visualisations | Makes complex scientific concepts and relationships easier to understand. |
| Verification and validation evidence | Documents the accuracy, limitations, and applicability of scientific implementations. |
| Human reviews and corrections | Preserves critiques, discovered errors, explanations, and improvements. |

These artifacts are intended to complement and connect with one another.

A user may begin by reading a scientific paper, examine the corresponding numerical implementation, explore its behaviour through an interactive simulation, and inspect the verification evidence supporting its results.

Discrepancies between literature, implementations, and simulation results become opportunities for further investigation rather than conclusions to be accepted without question.

Knowledge-management components such as KOVAN support this process, but the vision applies to **the entire OUTRAM PARK ecosystem**, including its scientific libraries, simulation engines, educational applications, documentation, and verification infrastructure.

### Preserving Knowledge Through Open-Source Development

OUTRAM PARK seeks to preserve the scientific and technical knowledge accumulated throughout software development, literature review, simulation, and human verification.

Even when individual implementations are superseded, corrected, or abandoned, the understanding gained from studying and challenging them can remain useful.

By retaining source code, documentation, references, verification results, and human corrections in an open-source ecosystem, OUTRAM PARK aims to make this accumulated knowledge available for future researchers, educators, students, and contributors.

The enduring outputs of OUTRAM PARK are therefore not limited to scientific software. They also include:

1. **Human expertise** developed through studying, challenging, and verifying scientific implementations.
2. **Accessible scientific knowledge** represented through interconnected literature, software, simulations, and visualisations.
3. **Reusable scientific software** whose reliability can progressively improve through systematic human review and verification.

### Human Judgement Remains Central

Generative AI can accelerate the construction of scientific software and the presentation of scientific knowledge, but it does not replace scientific judgement.

The responsibility for evaluating mathematical assumptions, numerical correctness, physical validity, and engineering applicability remains with humans.

OUTRAM PARK therefore treats agentically generated artifacts as **starting points for human inquiry, not substitutes for human understanding**.

Its broader purpose is to make scientific knowledge easier to access, explore, question, and preserve, while creating opportunities for people to develop the expertise needed to challenge that knowledge.

**Scientific knowledge should not merely be presented or executed. It should be accessible for humans to study, question, test, and challenge.**

---

**Verification and responsible use:** Unless supported by specific documented verification and validation evidence, OUTRAM PARK implementations must be treated as unverified. Educational demonstrations are not suitable for nuclear facility operation, reactor control, safety-critical engineering, licensing, or emergency-response decisions.

The same text is kept in [`VISION.md`](VISION.md). Each crate's README says, in its section **"Knowledge in this crate"**, which of these artifacts that crate holds: its literature and upstream sources, equations, source code, simulations and visualisations, V&V evidence, and human reviews and corrections.

---

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
| Opus 4.8 | \$5.00 | \$25.00 | \$6.25 / \$10.00 | \$0.50 |
| Sonnet 4.6 | \$3.00 | \$15.00 | \$3.75 / \$6.00 | \$0.30 |

Cache reads (0.1× input) dominate in practice because Claude Code caches
aggressively, so a naive "all input × \$5" overestimates by 3–10×. **Assume a
conservative upper bound of ~\$600** for the work above.

### Comparison to a human worker

The work needs a rare skill combination: production Rust **and** numerical
methods **and** nuclear/chemical-engineering thermodynamics (IAPWS-IF97,
two-phase critical flow, HEM, sound-speed behaviour at the bubble point) **and**
an egui GUI migration. A specialist who can do all of this bills roughly
**\$120–250/hr** as a contractor (≈ \$90–150/hr fully loaded as an employee). The
output produced is plausibly **3–10 focused engineer-days** for such a person,
i.e. **\$2,400–\$12,000**.

So at a conservative \$600, the AI-assisted cost is roughly **5–25% of the
human-equivalent cost** for the same output, and compresses the calendar time
from weeks to days.

### The honest caveat

This is **augmentation, not replacement** — the value is *conditional on expert
review*. The AI did not work autonomously: a domain expert set the direction,
made the key calls (e.g. the interpolation-vs-sonic-fix decision in the choked-
flow work), supplied domain notes, and verified every result. The AI also
thrashed on the hardest problem (the x ≈ 0 fix took several iterations and
introduced regressions that had to be caught and undone). An unreviewed AI fix
to a thermodynamics solver that is *subtly* wrong could cost far more than \$600
downstream (bad simulation output, a wrong published number).

**Verdict:** worthwhile for this mix of work — bounded-tedious migration (ideal
AI territory) and exploratory debugging *with a checkable oracle* (the Zaloudek
validation tests). It is **not** a safe substitute on work with no verification
oracle, or if AI output is treated as correct-by-default. The \$600 buys
specialist-grade output at a fraction of specialist cost **provided** the expert
stays in the loop to verify it; the review is what makes the spend safe rather
than risky.

## Publications & citation

**The project and its agentic-porting methodology** — OpenFOAM → `outram-foam-*`,
the TAMPINES steam tables and the 1-D HEM choked-flow solver, with their
preliminary V&V (cavity, Sod shock tube, Moody HEM charts, Edwards blowdown):

> Ong, T. K. C., Wong, E. Y. H., & Xiao, S. (2026). *Agentic Porting,
> Construction and Initial Verification and Validation of Libraries within the
> Open Source Unified TRAnsient Multi-Phase Advanced Reactor simulation Kit
> (Outram Park) Part I: Thermal Hydraulics.* arXiv:2608.17504 [physics.comp-ph].
> <https://arxiv.org/abs/2608.17504>

```bibtex
@misc{ong2026outrampark1,
  title        = {Agentic Porting, Construction and Initial Verification and
                  Validation of Libraries within the Open Source Unified
                  TRAnsient Multi-Phase Advanced Reactor simulation Kit
                  (Outram Park) Part I: Thermal Hydraulics},
  author       = {Ong, Theodore Kay Chen and Wong, Ethan Yew Hoe and Xiao, Sicong},
  year         = {2026},
  eprint       = {2608.17504},
  archivePrefix= {arXiv},
  primaryClass = {physics.comp-ph},
  url          = {https://arxiv.org/abs/2608.17504}
}
```

Its central finding is the premise this repository is organised around: with
agentic porting, **verification and validation by human expertise — not code
generation — is the bottleneck.** That is why every crate carries a
`## Bookkeeping status` block gated on maintainer sign-off, why
`VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md` treat AI output as
untrusted draft material, and why `docs/human-corrections-to-ai-work.md` exists.

The paper's line-count accounting is reproducible from this repository:
`kovan-cli kloc` regenerates it, and `docs/kloc-parity-baseline/` holds the
frozen fixture every published figure is checked against.

**The TUAS solver** (peer-reviewed; V&V against SAM, CIET forced-circulation
experimental data, and a HITEC-salt/YD-325-oil shell-and-tube heat exchanger):

> Ong, T. K. C., Xiao, S., & Peterson, P. F. (2025). *An open-source
> Thermo-hydraulic Uniphase Advection and Convection Solver for Salt Flows
> (TUAS).* International Journal of Advanced Nuclear Reactor Design and
> Technology. <https://doi.org/10.1016/j.jandt.2025.03.006>

See [`crates/tuas_boussinesq_solver/README.md`](crates/tuas_boussinesq_solver)
for the BibTeX entry and the V&V records that reproduce it.

> **Note on the acronym.** Part I's title expands OUTRAM PARK as
> "Multi-**Phase**". **As of 2026-09-12 the project is "Multi-Physics"** —
> multiphase flow is one crate of ~37, while the workspace spans neutronics,
> nuclear data, fuel performance, structural mechanics, DEM and more. The
> letters `M` and `P` are unchanged, so the acronym still spells out and no
> citation breaks.
>
> Part I stands as published — **cite its title verbatim, do not modernise it.**
> Part II onward use "Multi-Physics" and state the change where the acronym is
> first expanded, so a reader meeting both expansions is never left guessing
> which is a typo. Rationale and wording:
> [`docs/ecosystem-naming.md`](docs/ecosystem-naming.md).

## Crates

~~The workspace has 31 member crates, grouped by domain below.~~ **CORRECTED 2026-11-27:** the workspace has **51** member crates (counted from `crates/` and the root `Cargo.toml` members). The tables below listed 31; the 20 added since are in the last table, **"Added since the tables above were written"**. Each crate's README has a section **"Knowledge in this crate"** saying which knowledge artifacts it holds (see [Vision](#vision-scientific-knowledge-that-can-be-studied-and-challenged)), and the roster with maturity marks and dependency edges is [`docs/claude-md/members-and-dependencies.md`](docs/claude-md/members-and-dependencies.md). The roles below are short summaries; **a crate's own README and `AGENTS.md` are the authority for its status**, and the older rows were not each re-checked against their crates here.

**Thermal-hydraulics, fluid properties & process control**

| Crate | Role | License |
|---|---|---|
| [`chem-eng-real-time-process-control-simulator`](crates/chem-eng-real-time-process-control-simulator) | PID / transfer-function process-control library for real-time simulators | AGPLv3 |
| [`tuas_boussinesq_solver`](crates/tuas_boussinesq_solver) | Thermal-hydraulics Boussinesq single-phase solver (TUAS) | AGPLv3 |
| [`tampines-steam-tables`](crates/tampines-steam-tables) | IAPWS-IF97 steam/water properties + steam-turbine & choked-flow equations (TAMPINES) | AGPLv3 |
| [`tampines`](crates/tampines) | Central thermal-hydraulic framework that composes the TH crates | AGPLv3 |
| [`outram-park-fork-coolprop`](crates/outram-park-fork-coolprop) | Pure-Rust fork of CoolProp — Helmholtz-EOS thermophysical properties (independent fork, not official CoolProp) | AGPLv3 |
| [`outram-park-fork-offbeat`](crates/outram-park-fork-offbeat) | Pure-Rust fork of OFFBEAT — nuclear fuel performance: mechanics, rheology, gap/contact, material correlations, burnup/FGR, corrosion (independent fork, not official OFFBEAT) | AGPLv3 |
| [`farrer-park`](crates/farrer-park) | FEM structural mechanics — small-strain elasticity and J2 plasticity on Lagrange elements, from MOOSE / PRISMS-Plasticity / PRISMS-Fatigue (all LGPL-2.1). Uses `outram-foam-basic-lib` for the shared Krylov backend only, never for its discretisation. Verification only, no human V&V — not a life-assessment tool (independent fork, not affiliated with INL/MOOSE or the PRISMS Center) | AGPLv3 |
| [`outram-park-fork-dwsim-libs`](crates/outram-park-fork-dwsim-libs) | Pure-Rust fork of DWSIM process-simulation building blocks (independent fork) | AGPLv3 |

**CFD (OpenFOAM translations)**

| Crate | Role | License |
|---|---|---|
| [`outram-foam-basic-lib`](crates/outram-foam-basic-lib) | Pure-Rust translation of the OpenFOAM primitive + finite-volume layer (Layers 1–4): tensor algebra, polynomial/ODE solvers, interpolation, FV operators (`fvm`/`fvc`, MUSCL), thermophysics kernels, fields, mesh (independent fork, not official OpenFOAM) | AGPLv3 |
| [`outram-foam-turbulence-lib`](crates/outram-foam-turbulence-lib) | RAS/LES turbulence closures (k-ω SST implemented; others scaffolded) on top of `outram-foam-basic-lib` | AGPLv3 |
| [`outram-foam-appbuilder-lib`](crates/outram-foam-appbuilder-lib) | Solver application layer (pimpleFoam / rhoCentralFoam / rhoPimpleFoam) + case I/O; host of the in-progress GeN-Foam deterministic-neutronics + TH port | AGPLv3 |
| [`outram-foam-cli`](crates/outram-foam-cli) | OpenFOAM-style command-line utilities (blockMesh, pimpleFoam, gen-foam, …) as terminal binaries (independent fork, not official OpenFOAM) | AGPLv3 |
| [`outram-foam-multiphase`](crates/outram-foam-multiphase) | Phase-II multiphase CFD — drift-flux (Euler-Euler two-fluid, wall boiling, CHF, dryout planned); scaffold, no human V&V (independent fork, not official OpenFOAM) | AGPLv3 |

**Mesh generation & authoring**

| Crate | Role | License |
|---|---|---|
| [`outram-blender`](crates/outram-blender) | Fork of Blender's mesh-authoring architecture — headless surface-mesh frontend with opt-in Monte Carlo (`mc-export`) and OpenFOAM volume-meshing (`foam-mesh`) solver bridges (not affiliated with the Blender Foundation) | AGPLv3 |
| [`outram-park-fork-cfmesh`](crates/outram-park-fork-cfmesh) | Pure-Rust fork of cfMesh — Cartesian/tetrahedral/polyhedral volume meshing with boundary layers; consumes an `outram-blender` surface and emits an `outram-foam` polyMesh (independent fork, not official cfMesh) | AGPLv3 |
| [`outram-foam-mesh`](crates/outram-foam-mesh) | OpenFOAM mesh generation & conversion (blockMesh, snappyHexMesh, ideasUnvToFoam, polyDualMesh) (independent fork, not official OpenFOAM) | AGPLv3 |

**Neutronics & nuclear data**

| Crate | Role | License |
|---|---|---|
| [`teh-o-prke`](crates/teh-o-prke) | Point Reactor Kinetics (PRKE) for the Teh-O transport/eigenvalue solver | AGPLv3 |
| [`njoy-outram-park-fork`](crates/njoy-outram-park-fork) | NJOY2016 ENDF port — all nuclear data (RECONR/BROADR/THERMR/ACER/…, ν̄/χ, windowed multipole); exposes `XsProvider` (independent fork, not official NJOY) | AGPLv3 |
| [`outram-mc-libs`](crates/outram-mc-libs) | Monte Carlo transport — CSG geometry, particle tracking, k-eigenvalue, Woodcock/delta tracking, depletion; data-free (pulls cross sections from `njoy-outram-park-fork`) | AGPLv3 |
| [`boon-lay`](crates/boon-lay) | TRISO-particle / Lagrangian decay simulator (BOON-LAY); includes the TRISO-ATOPS fork | AGPLv3 |
| [`nee_soon`](crates/nee_soon) | Integration / coupling layer — composes MC + deterministic/TH + nuclear data + PRKE | AGPLv3 |
| [`outram-park-fork-liggghts`](crates/outram-park-fork-liggghts) | Pure-Rust granular-DEM library — particles, contact mechanics, thermal DEM, pebble/packed-bed physics (ports LIGGGHTS/LAMMPS-granular; GPL-2-or-later, see NOTICE); scaffold | AGPLv3 |

**Subsurface & infrastructure**

| Crate | Role | License |
|---|---|---|
| [`outram-park-fork-pflotran`](crates/outram-park-fork-pflotran) | Pure-Rust fork of PFLOTRAN — subsurface flow & reactive transport, no PETSc/MPI/FFI; scaffold, no human V&V (independent fork) | AGPLv3 |
| [`outram-park-mpi`](crates/outram-park-mpi) | Pure-Rust MPICH subset — the MPI-3 API surface over a shared-memory threads-as-ranks transport, Android-buildable, no C/FFI; scaffold (not affiliated with MPICH) | AGPLv3 |

**Digital twin**

| Crate | Role | License |
|---|---|---|
| [`outram-park-digital-twin-engine`](crates/outram-park-digital-twin-engine) | Offline digital-twin engine + egui GUI example simulators (offline demonstrations only) | AGPLv3 |

**KOVAN knowledge layer** (offline / Android-first)

| Crate | Role | License |
|---|---|---|
| [`kovan-common`](crates/kovan-common) | Shared canonical KOVAN types (`KovanDocument`, `KovanSymbol`, …) | AGPLv3 |
| [`kovan-discovery`](crates/kovan-discovery) | File discovery + text search (`ignore`/ripgrep) and git-awareness (`gix`) | AGPLv3 |
| [`kovan-literature`](crates/kovan-literature) | Literature archive — PDF → Markdown → `KovanDocument` → BibTeX | AGPLv3 |
| [`kovan-semantics`](crates/kovan-semantics) | Repo understanding — ripgrep-first, escalating to language servers | AGPLv3 |
| [`kovan-codegen`](crates/kovan-codegen) | Deterministic code generation for known numerical methods | AGPLv3 |
| [`kovan`](crates/kovan) (on crates.io: `knowledge-oriented-vv-analysis-for-nuclear-sciences-kovan`) | CLI (agent-facing, binary `kovan`), TUI (`ratatui`, binary `kovan-tui`, CLI-redirect stub on Android) and GUI (binary `kovan-gui`, reuses `kovan-literature`'s digitiser window) — three front ends over the KOVAN knowledge layer | AGPLv3 |


**Added since the tables above were written** (2026-11-27; roles summarised from each crate's own `Cargo.toml` description)

| Crate | Role | License |
|---|---|---|
| [`bedok`](crates/bedok) | Systems-level multiphysics coupling: 3-D nodal-diffusion neutronics coupled to thermal-hydraulics; a Rust translation of Than Yan Ren's (SNRSI) MATLAB implementation | AGPLv3 |
| [`bishan`](crates/bishan) | Level 2 PSA (in-plant severe-accident progression, containment and building response); first code a lumped HTR-10 reactor-building control volume, the rest not implemented | AGPLv3 |
| [`buangkok`](crates/buangkok) | Radiation dose and biological effects: HTR-10 dose tables (Liu and Cao 2002) as reference data and a code-to-code verified port of pyDOSEIA (MIT) | AGPLv3 |
| [`bugis`](crates/bugis) | The workspace's central GUI crate: Gruvbox theming, colour maps, egui widgets, the app scaffold and web-demo framework; `no_std` + `alloc` | AGPLv3 |
| [`changi`](crates/changi) | Atmospheric dispersion, plume transport, deposition and ground contamination; pure-Rust port of FLEXPART | AGPLv3 |
| [`dhoby-ghaut`](crates/dhoby-ghaut) | OUTRAM PARK's GUI home: the guided high-fidelity simulation workbench, meshing and Monte Carlo studios, and the tutorial web demos | AGPLv3 |
| [`dover`](crates/dover) | Low-fidelity reactor models driven by TOML input decks, headless; first case steam methane reforming in a CSTR | AGPLv3 |
| [`kaki-bukit`](crates/kaki-bukit) | `no_std` port of CYCLUS and CYCAMORE (agent-based nuclear fuel-cycle simulation); scaffold, no human V&V (not affiliated with the Cyclus project) | AGPLv3 |
| [`kembangan`](crates/kembangan) | Materials damage under irradiation; first code the RustBCA binary-collision Monte Carlo, moved from boon-lay; dpa and property changes not implemented | AGPLv3 |
| [`kovan-metrics`](crates/kovan-metrics) | KOVAN repository accounting: per-commit API-token trailers and historian reports | AGPLv3 |
| [`kovan-web`](crates/kovan-web) | web-kovan: the read-only Code Review UI (code map, module trees, function rings, source panel, review bar), browser and native | AGPLv3 |
| [`outram-park-fork-moltres`](crates/outram-park-fork-moltres) | MSR neutronics + thermal-hydraulics on the outram-foam FV layer, formulation from Moltres (LGPL); scaffold, no human V&V (not affiliated with Moltres/ARFC) | AGPLv3 |
| [`outram-park-fork-onix`](crates/outram-park-fork-onix) | Port of ONIX (MIT): Bateman/CRAM depletion and fission-product inventory for the MSRE digital twin; scaffold, no human V&V | AGPLv3 |
| [`outram-park-fork-thermochimica`](crates/outram-park-fork-thermochimica) | Port of ORNL Thermochimica (BSD-3): molten-salt Gibbs-energy minimisation thermochemistry for the MSRE digital twin; scaffold, no human V&V | AGPLv3 |
| [`pasir-ris`](crates/pasir-ris) | Reserved home for an integrated risk and reliability GUI; placeholder, nothing implemented | AGPLv3 |
| [`petir`](crates/petir) | Core numerics ported from the GNU Scientific Library (polynomials, equations, transforms, integration, roots, RNG, depletion maths); `no_std` | AGPLv3 |
| [`punggol`](crates/punggol) | Radiation transport beyond neutrons: OpenMC photon samplers and decay-photon spectra, and the EGSnrc port (PEGS4, HATCH, interactions, transport loops); awaiting human V&V | AGPLv3 |
| [`raffles`](crates/raffles) | Uncertainty quantification and risk analysis: port of RAVEN's core (Apache-2.0), plus Bayesian updating, ABC and imprecise probability; implemented in part, no human V&V | AGPLv3 |
| [`redhill`](crates/redhill) | Groundwater and geological transport of radionuclides after deposition; placeholder, nothing implemented | AGPLv3 |
| [`sembawang`](crates/sembawang) | Severe accident and source term, orchestrating the offsite chain to CHANGI; TRISO release via boon-lay, a reduced Fukushima Daiichi Unit 1 model, unvalidated | AGPLv3 |

## Build

Requires a system BLAS (OpenBLAS on Linux), but only for test:

```bash
# Arch / EndeavourOS
sudo pacman -S openblas
# Debian / Ubuntu
sudo apt install libopenblas-dev
```

```bash
cargo build --release --workspace
cargo test  --release --workspace --lib --tests
```

## Generated documentation (`kovan`)

Documentation generation and repository accounting run through this workspace's
own `kovan` binary. There is no Python in either toolchain.

```bash
cargo build --release -p knowledge-oriented-vv-analysis-for-nuclear-sciences-kovan --bin kovan     # builds target/release/kovan
```

**Prerequisites, both mandatory** — a nightly toolchain and `rustdoc-md`.
`rustdoc-md` itself is an ordinary stable binary; the nightly requirement
belongs to rustdoc's `--output-format json`, which is still unstable. It is
build tooling only: nothing shipped needs nightly, and the workspace builds,
tests and publishes on stable.

```bash
rustup toolchain install nightly
cargo install rustdoc-md --locked
```

### Regenerating the API mirrors

Each crate carries `crates/<crate>/docs/<crate>-api.md`, a single-file Markdown
mirror of its public API generated from the doc comments. To regenerate:

```bash
kovan api-docs --all                     # refresh every crate that has a mirror
kovan api-docs --all --include-missing   # also create mirrors for crates with none
kovan api-docs outram-foam-basic-lib     # just one crate
kovan api-docs outram-mc-libs --private  # include private items
```

`--all` reports progress per crate and **does not stop at the first failure** —
one crate that fails to document says nothing about the other thirty-six. It
lists what failed at the end and exits non-zero.

Notes, if kovan is not yet on cargo and you are running in this github 
repo:

```bash
cargo run --release -p knowledge-oriented-vv-analysis-for-nuclear-sciences-kovan --bin kovan -- api-docs --all --include-missing
```

### Bundling docs for an external agent

`kovan agent-docs-gen` packs the API docs into a flat, upload-ready set of files
for a chat agent with a fixed context window (the upload dialogs take files but
not folders, so nothing is nested).

```bash
kovan agent-docs-gen --list                       # what exists, and what each costs
kovan agent-docs-gen --crates tampines-steam-tables,outram-park-fork-coolprop
```

It writes three tiers, so you upload only what you need: `_INDEX.md` (a ~3 KB
roster of every crate), `<crate>.index.md` (condensed signatures), and
`<crate>.api.md` (the full rustdoc). `AGENTS.md` carries the workspace's coding
rules. The command reports a per-file token estimate against a budget and says
how many optional files fit in the headroom.

Again, if kovan is not on cargo and you are running on github:

```bash
cargo run --release -p knowledge-oriented-vv-analysis-for-nuclear-sciences-kovan --bin kovan -- api-docs --all --include-missing
cargo run --release -p knowledge-oriented-vv-analysis-for-nuclear-sciences-kovan --bin kovan -- agent-docs-gen --out ~/Desktop/agent-docs.
```

As seen here, u are able to put this on the desktop.



### Running from anywhere

None of these need to be run from the workspace root. When `--root` is omitted
the workspace is discovered, in this order:

1. the current directory or any ancestor of it;
2. `~/outram-park-backend`;
3. `~/Documents/outram-park-backend`;
4. `~/Documents/research/outram-park-backend`;
5. otherwise, pass `--root <path>` — the error names every path it tried.

A directory qualifies only if it holds `crates/` **and** a `Cargo.toml`
declaring `[workspace]`, so a directory that merely shares the name is not
mistaken for it. An explicit `--root` always wins, and a wrong one is an error
rather than being quietly replaced by a search.

Generated output follows the same preference: `--out` if given, otherwise
`<workspace>/agent-docs` (which `.gitignore` already covers), otherwise
`~/Documents/agent-docs` or `~/agent-docs`. Every run prints the workspace and
the output directory it chose, with the reason.

### Ingesting literature (`kovan lit`)

Any document that **informs the code** — a correlation taken from it, a
benchmark value cited, a number in a doc comment — belongs in
`crates/kovan-literature`, catalogued with its provenance. Not in `~/Downloads`,
not loose in `reference-data/`. "Used" is the trigger, not "ingested".

**Decide the access tier before importing, from the document's own copyright
page.** This is the step that matters and the one that is easy to get wrong:

- **Where you downloaded it grants you nothing.** INIS, gen-4.org, OSTI and a
  lab's website all host documents they do not license you to redistribute.
  "Open access" on a record page means freely *readable*, not freely
  *redistributable*.
- **No licence statement means proprietary**, not open. Unsure is the same as
  no. That failure direction is recoverable; the other is a licence violation in
  a public repository, because `open/` is committed and this repository is
  public.
- Check the copyright line for a **text-and-data-mining / AI-training
  reservation**. Where present, catalogue metadata and factual findings only and
  do not extract the full text.

```bash
# 1. Stage the PDF. kovan_import/ is gitignored scratch.
mkdir -p kovan_import && mv ~/Downloads/paper.pdf kovan_import/

# 2. Read the copyright page and decide the tier BEFORE importing.
#    open/  -> committed and published.   proprietary/ -> gitignored.

# 3. Import into the tier you decided on.
kovan lit import kovan_import/paper.pdf \
  --json-out    crates/kovan-literature/open/papers/author2026topic.json \
  --markdown-out crates/kovan-literature/generated/markdown/open/author2026topic.md

# 4. Acceptance check: the metadata must round-trip.
kovan lit bibtex crates/kovan-literature/open/papers/author2026topic.json

# 5. Add an entry to crates/kovan-literature/CATALOGUE.md, then delete the
#    staged copy.
rm kovan_import/paper.pdf
```

`kovan lit outline <pdf>` prints the heading structure without importing, which
is the cheapest way to see what a document actually contains.

**Check the extracted metadata; it is frequently wrong.** Observed failures
include the article-type label, a journal running header, the PII string and the
Word source filename all taken as titles, editors taken as authors, and a scan
date taken as the publication year. `kovan lit bibtex` round-tripping cleanly is
the acceptance check — if the BibTeX is wrong, fix the JSON before cataloguing.

### Other `kovan` commands

```bash
kovan tokens report      # regenerate the local token-usage summary
kovan tokens query --from 010826 --to 140826
kovan historian --from 010826 --to 140826   # pre-merge accounting report
kovan kloc --check       # the paper's productivity accounting
```

## Publishing (mandatory crate order)

`cargo publish` resolves **all** dependencies — normal *and* dev — against
crates.io, so each crate can only be packaged once everything it depends on
(directly or as a dev-dependency) is already live. Publish in this order:

| # | Crate | Must be published after |
|---|---|---|
| 1 | `chem-eng-real-time-process-control-simulator` | — (no internal deps) |
| 1 | `outram-foam-basic-lib` | — (no internal deps) |
| 1 | `outram-park-fork-coolprop` | — (no internal deps; only `uom`/`approx`) |
| 2 | `outram-foam-turbulence-lib` | `outram-foam-basic-lib` |
| 2 | `tuas_boussinesq_solver` | — (only dev-dep `chem-eng…`; the former `outram-foam-basic-lib` dep was removed in 0.1.3) |
| 3 | `outram-foam-appbuilder-lib` | `outram-foam-basic-lib`, `outram-foam-turbulence-lib` |
| 3 | `teh-o-prke` | `chem-eng…` (real dep) + dev-deps `tuas…`, `chem-eng…` |
| 4 | `tampines-steam-tables` | `tuas…` + dev-deps `teh-o-prke`, `chem-eng…` |

> This table covers the core inter-dependent crates. The remaining members
> (`tampines`, `njoy-outram-park-fork`, `outram-mc-libs`,
> `outram-park-fork-dwsim-libs`, `boon-lay`, `nee_soon`,
> `outram-park-digital-twin-engine`, and the `kovan-*` crates) publish per their
> own dependency edges; not all have been published yet. As of 2026-07-16,
> `tuas_boussinesq_solver`, `tampines-steam-tables`, `outram-park-fork-coolprop`,
> `outram-foam-basic-lib`, and `outram-foam-turbulence-lib` are live on crates.io
> (`outram-foam-appbuilder-lib` publish pending).

Publish each from the workspace root with `cargo publish -p <crate>` (commit
first; `cargo publish` refuses a dirty tree). Internal deps are
`{ path = …, version = … }` in the root `[workspace.dependencies]`, so a
crate's pinned `version` must be bumped here whenever an upstream crate it
depends on is bumped.

> **Keep this list in sync.** Whenever crate dependencies or versions change,
> update this table (and the per-crate version pins in the root `Cargo.toml`)
> so the publish order stays correct.

`cargo publish --dry-run` for crates 2–6 will fail with "failed to select a
version" until their upstreams are live — that is expected, not a packaging
error.

## License

AGPLv3 (`AGPL-3.0-only`), every crate. Some crates contain files ported from
GPL-3.0 projects (OpenFOAM, GSL, SCRAM, …); those files keep their GPL-3.0
notice, and the crate ships `LICENSE.GPL-3.0` beside `LICENSE`. Details:
[`docs/relicensing-agpl-2026-10-20.md`](docs/relicensing-agpl-2026-10-20.md).

## Responsible Use

Outram Park is an open-source nuclear engineering simulation ecosystem for education, research, capability building, and verification and validation.

The project uses only open-source data, public literature data, and properly licensed public benchmark data. It does not use confidential, restricted, proprietary, operational, or unpublished facility data.

AI-assisted development may be used for coding, translation, refactoring, documentation, and test generation. AI-assisted outputs are treated as draft material and must undergo human review, licence provenance checks, testing, verification, and validation where applicable.

Outram Park is not intended for nuclear facility operation, reactor control, licensing decisions, emergency response, safety-critical decision-making, safeguards-sensitive analysis, or security-sensitive analysis.

Digital twin examples are offline education and research demonstrations only. They do not connect to live operational systems, plant systems, safety-critical infrastructure, institutional production systems, or restricted infrastructure.

Simulation results should be interpreted with care. Users are responsible for checking assumptions, numerical limitations, data provenance, validation status, error estimates, and applicability to their intended use.

For details, see [`RESPONSIBLE_USE.md`](RESPONSIBLE_USE.md) and [`CONTRIBUTING.md`](CONTRIBUTING.md).

## Changelog

Human-in-the-loop decision log — the maintainer's directions and sign-offs,
recorded for provenance. AI agents executed these; a human decided them. Code
remains **unverified until a V&V case demonstrates otherwise** (see the banner
above).

### 2026-07-15

**Vendoring & forks** (upstreams cloned into each crate's gitignored
`upstream_source/`, per the workspace vendoring rule):

- **GeN-Foam** (GPL-3.0, `652b3da`) vendored into `outram-foam-appbuilder-lib`;
  Rust port begun (point-kinetics slice verified against the inhour equation).
- **OpenFOAM core** (v2606, `481094fd`) vendored into `outram-foam-basic-lib` as
  the primitive-layer reference.
- **TRISO-ATOPS** (MIT, Idaho National Laboratory, `de374c8`) forked into
  `boon-lay` as `triso_atops_fork` — a Eulerian TRISO fission-product-release
  model complementing boon-lay's Lagrangian one. All non-GUI functionality
  ported; GUI excluded; MIT attribution preserved. **Decision:** incorporate
  `uom` into its activity-bookkeeping layer (explicit sign-off overriding the
  unit-convention guardrail).

**Digital-twin engine:**

- Renamed `outram-park-digital-twin-gui` → **`outram-park-digital-twin-engine`**.
- Roadmap set: rework/build `fhr_sim_v2`, `htgr_sim_v1`, `ipwr_sim_v1` on the
  engine's reusable widgets (fhr + htgr are the current priority; ABWR and others
  are future work). `fhr_sim_v2` migrated into the engine crate as the first
  consumer.

**Dependency policy:**

- **`ndarray-linalg` removed** from all non-benchmark code — the OpenFOAM-port
  pure-Rust dense-LU `SquareMatrix` replaces the sole remaining linear solve
  (`fhr_sim_v2` secondary-loop mass balance). It now survives only in the
  `outram-foam-basic-lib` matrix benchmark.
- Approved new pure-Rust, Android-friendly workspace dependencies for KOVAN:
  `regex`, `pdf-extract`, `lopdf`, `tempfile`.

**KOVAN knowledge layer:**

- Directed a best-effort implementation pass across all seven `kovan-*` crates
  (per-agent decisions in
  [`docs/kovan-agent-decisions-for-review.md`](docs/kovan-agent-decisions-for-review.md)).
- Approved: the `Method::Pde` codegen-catalogue addition; the `kovan-common`
  shared-type additions requested by downstream crates (`KovanSymbol` location
  fields, `KovanDocument` asset/page/journal-locator fields + builder,
  `GeneratedArtifact`); and keeping `kovan-discovery`'s `require_git(false)` fix
  (honour `.gitignore` outside a `.git` repository).

**TUAS validation:**

- Adopted the SAM-matched CIET coupled-DRACS **pipe-38 form loss K = 17.8**
  (validated on the isolated loop; shared constructor `new_pipe_38_sam_model`),
  lowering mean |DRACS error| vs experiment from **3.83 % to 2.76 %** and
  tightening every mid/high-flow case ~2 pp (matching SAM's NED-2021 Table-4
  predictions). The ~275 regression references were recomputed at K = 17.8 and
  **all 25 coupled cases pass**. A single uniform K cannot correct a bias that is
  over-prediction at high flow and under-prediction at low flow, so the two
  lowest-flow cases (b1 655 W, c1 841 W) — and, via reduced DHX heat removal, two
  high-power heater-surface-temperature bounds (b7, b9) — take **per-point,
  documented tolerance widenings** with in-source justification; **no global
  benchmark (mass-flow) tolerance was loosened**. The proper velocity-dependent
  pipe-38 loss model is deferred to bead `op-4wl.5`. Per-case plotting CSVs, plus
  41 redirected pre-existing test CSV writers, now land in the gitignored
  `verification_and_validation/` folder. This calibration is documented as a
  human-in-the-loop V&V decision (see `tuas_boussinesq_solver`'s README).
