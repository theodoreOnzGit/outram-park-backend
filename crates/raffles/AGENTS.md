# AGENTS.md — raffles

Guidance for any AI agent working in `crates/raffles`, whatever tool it runs
under. `crates/raffles/CLAUDE.md` points here; the rules below bind exactly as
if they were written there.

Where this file and the workspace-root `CLAUDE.md` differ, **this file wins for
this crate**, because it records maintainer direction specific to RAFFLES. The
root file still binds for everything this file does not address.

---

## Calibration is acceptable when it is the intention

**Maintainer direction, 2026-10-12:** *"Calibration is acceptable if that's the
intention, surrogate models by definition calibrate."*

### What this means in RAFFLES

- **A surrogate model is a calibrated model, and that is not a defect.** A
  polynomial regression, a Gaussian process (kriging), a polynomial chaos
  expansion or a neural network is *defined* by fitting its coefficients to
  data. Fitting is the method, and the fitted model is the product. Do not
  treat building one as a breach of any rule against calibration, and do not
  refuse or hedge a surrogate task on those grounds.
- **Bayesian model updating is calibration by design.** TMCMC, TEMCMC, plain
  MCMC and ABC exist to infer parameters from data, so the posterior *is* the
  calibration. Calibrating model inputs against measured data with them is
  their intended use.
- **This holds when RAFFLES is used on physics outputs too.** Example: fitting
  a surrogate to Monte Carlo `k_eff` results, or inferring uncertain HTGR
  inputs (packing fraction, graphite impurity) from measured criticality data.
  When calibrating is the stated purpose of the work, it is acceptable.

### How this relates to the root `CLAUDE.md`

The root file's "Model hierarchy" rule says to run a surrogate uncalibrated
first and calibrate only afterwards, with ablation testing. Its "Get the
process right" rule forbids tuning an input until a comparison passes. Both
rules target one failure: a fit dressed up as an independent check, where the
benchmark has silently become an input.

The maintainer's direction separates that failure from intended calibration:

- **Calibration is the intention:** acceptable in RAFFLES. Say that the model
  is calibrated and what it was calibrated against. Saying so is what makes
  the intention visible.
- **The comparison is meant to be a check** (a verification or validation gate,
  a benchmark the result is judged against): tuning to make that check pass is
  still forbidden. Calibrating to the very data that is supposed to test the
  model destroys the test.

## Avoid writing macros (workspace rule, 2026-11-05)

Maintainer direction: macros make code harder to read and review. Do not
write new `macro_rules!` or proc macros in this crate; use functions,
generics, enums and plain `match` arms. Using std and dependency macros
(`vec!`, `format!`, derives) is fine. Full rule: root `CLAUDE.md`, "Avoid
writing macros".

## Transcendentals go through PETIR, never the platform libm (HARD RULE, 2026-11-05)

Maintainer direction (gh:#1290): in this crate, call `exp`, `ln`, `log10`,
`powi`, `powf`, `sin`, `cos`, `atan2` and the rest through
`petir::mathf::RealMath` (`x.r_exp()`, `x.r_ln()`, `x.r_powi(n)`, ...),
never the inherent `f64` methods (`x.exp()`). The inherent methods take
their last bit from the platform's C library (glibc, MSVC, Apple, wasm), so
the same run gives different bits on different machines. PETIR's are one
implementation everywhere. A raw call still compiles and runs, so the
workspace gate `crates/petir/tests/no_platform_libm.rs` is what catches it;
it was red on 2026-11-05 with 40 raw calls in this crate and its sibling
(#1290). Keep it green.

## Crate guidance (this crate's `CLAUDE.md` until 2026-11-05)

The rest of this file was `crates/raffles/CLAUDE.md`, merged here on
2026-11-05. Where it says "[`AGENTS.md`](AGENTS.md)" it means the
sections above, which take precedence where they differ.


Crate-specific guidance for Claude Code and other AI assistants working in
`crates/raffles`. ~~The workspace-root `CLAUDE.md` still binds in full — this
file adds to it, and relaxes nothing.~~ **CORRECTED 2026-10-12** — the
workspace-root `CLAUDE.md` still binds, except where maintainer direction
recorded in [`AGENTS.md`](AGENTS.md) relaxes it for this crate.

**Read [`AGENTS.md`](AGENTS.md) first.** It holds maintainer direction that
binds every agent working here, and it takes precedence over this file and the
root `CLAUDE.md` where they differ. Currently it covers one topic:
**calibration is acceptable when it is the intention, and surrogate models
calibrate by definition** (maintainer direction, 2026-10-12).

**RAFFLES** — **R**isk **A**nalysis **F**ramework **F**or **L**earning &
**E**nsemble **S**imulation. An independent pure-Rust port of the
uncertainty-quantification and risk-analysis core of
[RAVEN](https://github.com/idaholab/raven) (Idaho National Laboratory,
Apache-2.0).

**Current state: implemented in part, with no human V&V.** Distributions,
samplers, sensitivity, Bayesian model updating, distances, ABC, imprecise
probability, model selection, fault-tree quantification, GNNs and surrogates
all carry working, tested code. All of it is AI-assisted draft material until
the maintainer and the crate owner have reviewed it, so do not describe any
part of it as validated, and read "verified" as "checked against a reference by
an automated test".

---

## Reactor geometry is DRAWN for a human to check before it is trusted (HARD RULE)

> **Note, 2026-10-02.** This crate took the rule below on through its
> dependency on `outram-mc-libs`, which it no longer has (see "RNG" below). The
> rule is left in force here — RAFFLES' graph work still consumes reactor
> geometry through `outram_blender::gnn_graph` and liggghts' `gnn_bridge` —
> and whether to drop it is the maintainer's call, not an assistant's.

**Maintainer direction, 2026-09-25.** Binds this crate. The same rule is in the
`CLAUDE.md` of `outram-mc-libs`, `nee_soon`, every `outram-foam-*` crate and
every crate downstream of them; a crate that newly depends on one of those
takes the rule into its own `CLAUDE.md` (check with `cargo metadata`).

**Whenever you build or change a complex reactor geometry** — CSG cells and
surfaces, lattices, pebble beds, TRISO particles, reflector zones, control-rod
bands, a CFD/FEM mesh, anything a solver will transport or integrate through —
**draw it, as images a human can open (PNG, or JPG/SVG), and hand them over**
before any result computed on it is reported as more than tentative.

- **Draw what the solver sees, not what you meant.** Render from the ASSEMBLED
  geometry (cell / material lookup at each pixel, or the mesh itself), never
  from the named constants. A picture of the constants hides exactly the
  defects this rule exists to catch.
- **Minimum set:** an axial slice (R-Z / x-z) of the whole model; radial (x-y)
  slices at the heights that matter; and, for nested geometry, zoomed slices at
  every level down to the smallest (pebble, TRISO particle). Colour by
  material, with a legend and the key dimensions marked.
- **Commit the images with the change** (beside the V&V record or the
  manuscript package) and point the human at them by path in your summary.
  Regenerate them whenever the geometry changes.
- **Say what you checked in them, and what you could not** — an image nobody
  was told to look at checks nothing.

**Why.** On 2026-09-24/25 the HTR-10 model carried, at once: a bottom reflector
mirrored from the top and up to 107 cm short; a core cavity that grew with the
bed; pebbles interpenetrating by 1.1 cm with 4.8 % of core carbon clipped away
(gh:#309, #310); and a TRISO lattice holding 8240 particles while reporting
8340 (gh:#316, +353 pcm). Every run completed with green diagnostics. Each was
found by looking at the built geometry, not by the eigenvalue.

**Tools.** ~~`outram_mc_libs::geometry::plot` samples a slice of an assembled
CSG geometry; OpenMC-parity image output (PNG/JPG) is the preferred path once
it lands.~~ **UPDATED 2026-09-25 — it has landed (gh:#268):**
`outram_mc_libs::geometry::plot` is a port of OpenMC's plotter that writes PNG
directly — slices, wireframe and solid ray traces — verified pixel-for-pixel
against `openmc --plot`
(`crates/outram-mc-libs/verification_and_validation/geometry_plotting/`).
`render_material_slice` draws a material-coloured slice with a legend and cm
axes in one call; `crates/nee_soon/examples/htr10_geometry_images.rs` is the
worked example. For meshes, plot the mesh itself (cells, patches, zones).

## Ownership — Adolphus Lye

**CORRECTED 2026-11-04 (maintainer direction): "Adolphus should be okay,
entirety of outram park is mine also to own (bedok is Yan Ren's)."** The
workspace maintainer owns this crate, as every crate in the workspace except
`bedok` (Than Yan Ren's). **The direction calls below recorded as "theirs to
confirm or reverse" (`scram/`, `estimators`, its submodules, and
`raffles::fidelity::build` running the caller's model, gh:#1194) are confirmed
by the maintainer**, and new direction from the maintainer needs no further
sign-off. Adolphus Lye remains the author of the backronym and of the Bayesian
work the crate draws on; credit him accurately (he/him). The text
below is kept struck through as the history of the earlier position.

~~**This crate belongs to Adolphus Lye**, a colleague of the workspace
maintainer. They chose the RAFFLES backronym, and **direction, scope and
priorities for this crate are theirs to set.**~~

What that means in practice:

- A change of direction — adding or dropping a module, changing what the crate
  is for, restructuring the public API, taking on optimisation or the
  simulation-driver layer that is currently out of scope — is **their call, not
  yours and not another agent's**. Propose it to them; do not decide it.
- If a task you are given conflicts with what this file or the README records
  as the crate's scope, say so and ask, rather than quietly widening the scope.
- Attribute their ownership accurately in anything you write about the crate.
- ~~**Use they/them for Adolphus Lye.** Their pronouns have not been stated, so
  singular *they* is the correct default here — do not guess otherwise.~~
  **CORRECTED 2026-11-04 (maintainer): "Adolphus is he."** Use he/him for
  Adolphus Lye. Older text in this crate that says "they" for him is not wrong
  enough to rewrite; new text uses he/him.

Filling in a module the crate already declares, to the design rules below, is
ordinary contribution and does not need a fresh decision.

---

## Upstream provenance

| | |
|---|---|
| Project | RAVEN (Risk Analysis Virtual ENvironment) |
| Developer | Idaho National Laboratory (INL) |
| Repository | <https://github.com/idaholab/raven> |
| Licence | **Apache-2.0** |
| Copyright | `Copyright 2017 Battelle Energy Alliance, LLC` |
| Branch / commit referenced | `devel` @ `01216937967c38ee287859270c035c8eca906dc6` (2026-07-14) |
| Latest release at time of access | `RAVENv3.2` (2026-03-12) |
| Date accessed | 2026-08-06 |

Verbatim upstream files preserved in this crate: `LICENSE-APACHE-RAVEN`
(upstream `LICENSE.txt`) and `NOTICE-RAVEN` (upstream `NOTICE.txt`). Both were
fetched, not retyped. Do not reformat, truncate or "tidy" them.

The crate `NOTICE` is the authoritative provenance record.
`upstream_source/README.md` holds the clone command and the map from RAFFLES
modules to upstream paths.

### Apache-2.0 into ~~GPL-3.0~~ AGPL-3.0 is ONE-WAY — the rule you must not get wrong

RAFFLES is ~~**GPL-3.0-only**~~ **AGPL-3.0-only** (relicensed 2026-10-20, gh:#973), an AGPL-3.0-only combined work under GPLv3 §13 / AGPLv3 §13: its GPL-3.0 ports (SCRAM, Physics-guided-MPNN, pba.r) keep GPL-3.0, and the crate ships `LICENSE.GPL-3.0`. Apache-2.0 is one-way compatible with GPLv3 and AGPLv3 alike:

- Code may flow **RAVEN (Apache-2.0) into RAFFLES (~~GPL-3.0-only~~ AGPL-3.0-only)**.
- Code may **not** flow **RAFFLES (~~GPL-3.0-only~~ AGPL-3.0-only) into RAVEN (Apache-2.0)**.

Never contribute RAFFLES code, patches or translated files upstream to RAVEN,
never open a pull request against `idaholab/raven` out of this workspace, and
never offer RAFFLES code to anyone under Apache-2.0 terms. Once a translation
lands here it is ~~GPLv3 and stays GPLv3~~ AGPLv3 (or GPLv3, for a GPL-upstream port) and stays so. Relicensing would require every RAFFLES
copyright holder's agreement — which no AI assistant may arrange, and no
contributor may arrange unilaterally.

This restricts the direction of *code* flow only. Reading RAVEN's papers,
theory manual and documentation and implementing the published algorithms is
unaffected — and where the algorithm is published, an independent
implementation from the paper is usually the better route anyway.

### SCRAM is a second upstream, and it is NOT Apache-2.0

`src/scram/` is ported from **[SCRAM](https://github.com/rakhimov/scram)**
(Olzhas Rakhimov), commit `b85b78940de38996eeffec54d946824bd4280a1c`,
accessed 2026-09-21. SCRAM is **GPL-3.0-or-later** — ~~the same licence family~~ **CORRECTED 2026-10-20**: since the AGPL relicence (gh:#973) the SCRAM ports stay GPL-3.0 and combine with the AGPL crate under §13; originally the same licence family
as this crate — so it carries **none** of the one-way constraint the RAVEN
grant does, and the Apache-2.0 header template below is **wrong** for those
files. Use the GPL header the existing `src/scram/` files carry.

**Part of `src/scram/` is a port and part deliberately is not, and the
difference is load-bearing.** Ported, with attribution headers:

| file | upstream |
|---|---|
| `probability.rs` | `RareEventCalculator`, `McubCalculator`, `CutSetProbabilityCalculator` |
| `importance.rs` | `ImportanceAnalyzerBase::Analyze`'s five derived factors |
| `bdd.rs` (the probability recurrence only) | `ProbabilityAnalyzer<Bdd>::CalculateProbability` |
| `zbdd.rs` | `ConvertBdd`, `Minimize`, `Subsume`, `ConvertBddPrimeImplicants`, `Bdd::Consensus`, `ConvertGraph`, `Apply<kAnd>`, `Apply<kOr>`, `EliminateComplements` |
| `fault_tree.rs`'s `Connective` taxonomy | `pdag.h`'s `enum Connective` |
| `expression.rs` | `src/expression/*.cc` — `p_exp`, GLM, Weibull, periodic test, the numeric and Boolean operators, and the seven random deviates' `value`/`Validate`/`interval` |
| `mef.rs` | `initializer`, `xml`, `element`, `model`, `fault_tree`, `event` — the MEF reader, including `Initializer::GetEntity`'s name resolution and `Pdag::ConstructComplexGate`'s rewrites |
| `ccf.rs` | `ccf_group.{h,cc}` — all four common-cause models, `CalculateProbabilities`, the combination reciprocal and `ApplyModel`'s proxy-gate rewrite |
| `alignment.rs` | `alignment.{h,cc}` and the phase application of `RiskAnalysis::RunAnalysis` — mission-time scaling and `<set-house-event>` |
| `uncertainty.rs` | `uncertainty_analysis.{h,cc}` — the Monte Carlo loop and every statistic, including the `n/(n-1)` variance correction |
| `substitution.rs` | `substitution.{h,cc}`, plus `Pdag::ConstructSubstitution` (declarative, a tree rewrite) and `Zbdd::ApplySubstitutions` (non-declarative, a pass over products) |

**Not** ports, each saying so in its own doc instead:

- `mocus.rs` — upstream's MOCUS drives a ZBDD over a preprocessed Boolean
  graph (`zbdd` + `pdag` + `preprocessor` + `bdd` = 9,076 lines). This is the
  classical top-down expansion from the literature, kept as an unrelated
  second opinion now that `zbdd.rs` is the scalable path.
- The exact top-event probability by cut sets — inclusion-exclusion, not a BDD
  traversal.
- The Birnbaum factor — its definition, not a BDD derivative.
- `bdd.rs`'s diagram construction — Bryant's algorithm from the tree, where
  upstream builds from a preprocessed `Pdag`.
- The rest of `fault_tree.rs` — a plain indexed structure, not upstream's
  `Initializer`/`Model`/`Formula`. `mef.rs` is what builds one from XML.
- `mef.rs`'s **schema validation**, because there is none. Upstream validates
  against `share/input.rng` with libxml2 first; this checks only what it needs
  to build the model. Every construct it does not understand is an error
  rather than a skip, which is what stands in for the grammar.

**Do not retrofit an attribution header onto any of those.** A header is a
statement about where a file came from, and the whole value of them is that
they are *independent* of upstream: two unrelated algorithms agreeing is
evidence, a translation agreeing with its original is much weaker. That is the
same rule the paper-derived Bayesian modules follow.

**SCOPE WIDENED AGAIN, 2026-09-22 (maintainer direction):** *everything
except the GUI* is to be translated, with V&V. That brings XML input, event
trees, alignments, CCF groups, substitutions, the expression library,
`define-component` namespaces and the preprocessor **into** scope — all of
which this file and the README previously recorded as out of it. The memory
cap stays an exception (the largest Aralia benchmarks remain out), and the
non-BDD ZBDD constructor was named explicitly.

~~Still to do under that direction: XML input (`initializer`, `xml`), the
`expression` library, …~~ **CORRECTED 2026-09-22** — the first two landed:
`src/scram/expression.rs` and `src/scram/mef.rs`. Still to do:
event trees and sequences, the preprocessor, `pdag`, and the reporter. Progress is tracked in `docs/scram-port-verification.md`.

~~expressions~~ **CORRECTED 2026-09-22** — `src/scram/expression.rs` landed,
verified on `HIPPS`, upstream's own model whose every basic event is defined
by a `<periodic-test>` or `<GLM>` expression and whose probabilities therefore
appear nowhere as literals.

~~XML input, `<define-component>` private namespaces~~ **CORRECTED
2026-09-22** — `src/scram/mef.rs` landed. It removed the transcription step
that stood between upstream's models and the tests: `tests/scram_mef.rs` reads
upstream's own `.xml` (committed verbatim under
`reference-data/scram/upstream-input/`), evaluates every basic-event
probability from the model's own expressions, and checks the cut sets, totals
and importance factors that follow. **`ThreeMotor/three_motor` is covered end
to end for the first time** — the model that no route reached, because its
`<define-component role="private">` declares a second `E1`. Name resolution is
upstream's `Initializer::GetEntity` rule for rule, and two details it is easy
to get wrong are asserted separately: a `<define-fault-tree>` **is** a path
component, and a component's role **inherits** its container's rather than
defaulting to private.

**A claim this port made about the format was wrong, and is corrected.** An
earlier `mef.rs` carried machinery for arbitrarily nested formulas. MEF has
none: `share/input.rng` lets a connective take only an event reference, a
`<not>` around one event, or a `<constant>`. SCRAM itself rejects a nested
formula. Do not re-add that machinery.

~~the random deviates (`SmallTree/SmallTree` and `BSCU/BSCU` are the two
upstream models this costs)~~ **CORRECTED 2026-09-22** — all seven landed, and
both models read. ~~Only their deterministic `value()` is ported~~ **CORRECTED 2026-09-22** —
`Expression::sample` and `src/scram/uncertainty.rs` landed together, which is
the order that made the sampling checkable at all.
**The random stream differs from upstream's** (one static `std::mt19937`
there, `petir::rng::lcg` here (~~`outram_mc_libs::rng::lcg`~~, the same
generator before it moved on 2026-10-02), reused per the
search-before-building rule), so this is the **one part of the SCRAM port
whose verification is statistical rather than exact**. Do not tighten
`scram_uncertainty`'s tolerances into exact comparisons: they are set by the
sample sizes, and SCRAM's own 1000 trials dominate every band. Their arrival is also what gave
`Interval`/`Expression::interval` a purpose — a normal deviate's mean can be a
good probability while its six-sigma domain is not, and upstream rejects the
argument on the domain check.

~~CCF groups~~ **CORRECTED 2026-09-22** — `src/scram/ccf.rs` landed, porting
all four models (beta-factor, MGL, alpha-factor, phi-factor) and
`ApplyModel`'s proxy-gate rewrite. **They are applied BY DEFAULT**, not behind
a flag as upstream's `--ccf` is, per the workspace rule that physics the data
supplies is applied unless a caller ablates it; `MefModel::without_ccf` is the
visible ablation and both paths are pinned against the corresponding SCRAM
run. The default is asserted by
`scram_ccf::ccf_is_applied_by_default_and_the_ablation_is_the_independent_analysis`
— do not turn it off. Only upstream's `beta-factor` appears in any upstream
input model, so `models-for-this-port/ccf_models.xml` carries all four.

~~substitutions~~ **CORRECTED 2026-09-22** — `src/scram/substitution.rs`
landed. Both upstream models are checked end to end, and the work turned up a
**defect in upstream**: `ImportanceAnalyzer<Bdd>::CalculateMif` omits the
root-complement negation that `CalculateTotalProbability` applies, so every
Birnbaum factor of an affected model comes out with the wrong sign. This had
been recorded as an unexplained 108-event divergence on `Aralia/das9601`;
`TwoTrain/substitutions` reproduces it on six events, small enough to check by
hand, and two hand calculations in opposite directions land on this port's
sign. **Do not "fix" the sign to match SCRAM.**

~~alignments~~ **CORRECTED 2026-09-22** — `src/scram/alignment.rs` landed.
A model with an alignment has **no single answer**: `MefModel::in_phase`
returns the model as it stands in one phase and the caller loops, which is
upstream's own arrangement. Upstream mutates the model and restores it with a
`scope_guard`; this returns a new model, so two phases cannot interfere.

Still absent, and refused rather than skipped when a model uses them:
event trees, the trigonometric operators and `<switch>`.
`<define-extern-function>` is refused **deliberately and permanently** — it
loads a shared library named by the input file, which the workspace
`RESPONSIBLE_USE.md` rule on autonomous access forbids.

~~house events~~ **CORRECTED 2026-09-21** — `Arg::Constant` and
`FaultTreeBuilder::house_event` landed the same day. Upstream's only
house-event model, `ThreeMotor`, also uses `<define-component>` (private
namespaces the structure extractor does not model), so a small model was
written to give the feature real oracle coverage rather than repeat
`Connective::Null`'s position of shipping unverified.

~~prime implicants~~ **CORRECTED 2026-09-21** — `zbdd::prime_implicants`
landed the same day, porting `ConvertBddPrimeImplicants` and `Bdd::Consensus`.
It is what makes a non-coherent answer *exact* rather than conservative.
Verified against `scram --prime-implicants` on 9 models, 441 implicants, signs
included. Note upstream itself does not finish `--prime-implicants` on
`Aralia/das9601` within five minutes — the cost is the algorithm's, not this
port's.

~~ZBDD~~ **CORRECTED 2026-09-21** — `src/scram/zbdd.rs` landed the same day,
porting `ConvertBdd`/`Minimize`/`Subsume`. It is how `Aralia/das9601`'s 4,259
cut sets are verified: `mocus` cannot reach them at any order limit, and the
ZBDD produces all of them in under a second.

~~BDD~~ **CORRECTED 2026-09-21** — `src/scram/bdd.rs` landed the same day: a
Bryant-style diagram built straight from the tree, with upstream's probability
recurrence ported verbatim. It removed the largest limitation the port had —
exact probability no longer needs cut sets, so it is no longer capped at 20 of
them. Verified against SCRAM's own BDD on 10 models, including `Aralia/das9601`
(288 gates, non-coherent, 386,261 nodes) which `mocus` cannot touch.

~~complement elimination (so **non-coherent trees are refused, not
approximated**)~~ **CORRECTED 2026-09-21** — complement elimination landed the
same day. `mocus` now expands negated gates through their De Morgan duals,
drops contradictory partial sets, deletes the complements and re-minimises,
matching what upstream's `Zbdd::EliminateComplement` does. **The answers mean
something different, though**: minimal cut sets of a non-coherent tree are
conservative, so quantifying them is an upper bound, not the probability.
Measured on the fixture's small non-coherent model, ours is `0.622` against
SCRAM's exact BDD value of `0.5032` — `+23.6 %`, and correct. Do not "fix"
that gap.

~~upstream's probability cut-off on products~~ **CORRECTED 2026-09-21** — that
was listed as absent, wrongly. SCRAM 0.16.2 stores and validates
`Settings::cut_off_` but **never reads it**: the getter has no callers, and
`--cut-off 0.5` on a model whose top-event probability is `1.17e-3` discards
none of its 392 products. There is nothing to port, and truncating by cut-set
order only is not a divergence. Do not "fix" this by implementing one.

Verification record: [`docs/scram-port-verification.md`](docs/scram-port-verification.md),
oracle in `reference-data/scram/`. Upstream was **built and run** — nothing in
that fixture was reasoned out from reading SCRAM's source.

### Computing with Confidence and pba.r (gh:#987) — a third and fourth upstream

`src/scram/pba.rs` ports the pba.r operations that Adolphus Lye's "Coherent
systems" scripts call; `src/scram/pbox_tree.rs` evaluates a `FaultTree` with
c-box basic events. Provenance: Lye's scripts have no licence file (the author
asked for the port; maintainer confirmed AGPLv3 fine, 2026-10-20); `pba.r` is
Scott Ferson's, GPL-3.0, so `pba.rs` carries a pba.r header. **`sra.r` in the
same upstream folder is "All rights reserved": never read, port or run it.**
Verified against the original R scripts (R 4.6.1) and an Octave Monte Carlo:
`docs/computing-with-confidence-verification.md`, which also lists what is
NOT verified. Two traps recorded there: `KN` is `cbox_binomial` except at the
support ends and the `1e-6` clamp (`pba::kn` / `kn_exact`), and the published
Case study 4 overwrites its c-boxes with point values. AI-assisted draft, no
human or owner review.

### The gh:#1193 RAVEN routes, and their BSD-3-Clause numerics (2026-11-04)

Ported from RAVEN `devel` @ `8baaae3a97f1392318074ad6d3f8efc26ca81c7e`
(accessed 2026-11-04; `LICENSE.txt` and `NOTICE.txt` unchanged):
`src/bayesian/raven_mcmc.rs` (Metropolis, adaptive Metropolis),
`src/surrogate/polynomial_chaos/{mod,grid}.rs` (`GaussPolynomialRom`, index
sets, Smolyak/tensor grids, orthonormal polynomials) and the interface half of
`src/surrogate/gaussian_process.rs`. Where RAVEN delegates, the delegated code
is ported too, under **BSD-3-Clause**, with its own header and licence file:
SciPy 1.14.1 and NumPy 1.26.4 in `polynomial_chaos/special.rs`
(`LICENSE-BSD-SCIPY`, `LICENSE-BSD-NUMPY`), scikit-learn 1.3.2 in
`gaussian_process.rs` (`LICENSE-BSD-SKLEARN`). The crate `NOTICE` records all
four. The hyperparameter optimiser is **not** a port of L-BFGS-B.

Differential protocol and results:
`verification_and_validation/raven_routes_gh1193.md`, references generated by
running RAVEN and the libraries (`raven_routes_gh1193/generate_references.py`),
compared in `tests/raven_routes_gh1193.rs`. Two RAVEN behaviours are ported
faithfully and must not be "fixed" in the RAVEN mode, only offered beside it:
the **clamp at a prior bound** (`BoundHandling::ClampAsRaven`; with tuning it
absorbs the chain at the bound, measured in the record) and `min(0, NaN) = 0`
(a NaN ratio is accepted, counted in `nan_log_ratios`). `raven_mcmc` calls the
caller's likelihood, as `bayesian::transitional` already did; this is the
"model-in-the-loop sampler" the scope list below called out of scope, brought
in by the maintainer's gh:#1193 direction.

### The gh:#1190 SMT kriging routes (BSD-3-Clause, 2026-11-04)

`src/surrogate/smt/` ports SMT @ `151a3ff3c137` (`LICENSE-BSD-SMT`, NOTICE):
KRG, KPLS, KPLSK (`SmtKriging`, `SmtPls`) and MFK co-kriging (`Mfk`), with
the PLS from scikit-learn 1.9.1 (`pls.rs`). Typed access:
`fidelity::KrigingRoute::Smt`, `SurrogateSpec::CoKriging` and
`Surrogate::fit_multi`. Differential record:
`verification_and_validation/smt_kriging_gh1190.md`, tests
`tests/smt_kriging_gh1190.rs`. The optimiser is the crate's bounded BFGS,
not TNC. Three SMT behaviours are ported faithfully and must not be "fixed"
in the SMT mode: KRG's universal-kriging variance uses only `diag(G)`
(`TrendVariance::AsSmt`; `Exact` is offered beside it), KRG divides its
likelihood by n where MFK divides by n − p − q, and MFK's variance for three
or more levels uses `mu = 0` above level 1.

### The gh:#1180 Kennedy–O'Hagan route: SEPIA (BSD-3-Clause, 2026-11-04)

`src/bayesian/koh/{mod,data,model}.rs` port LANL's SEPIA @ `309890423a12`
(`LICENSE-BSD-SEPIA`, NOTICE): its scalar-output model y = η(x, t) + δ(x) +
ε (the GPMSA formulation), transforms, covariance, log-likelihood, priors
and dense predictor. `sample.rs` is not a port: raffles' TMCMC samples
SEPIA's posterior. The typed route is `fidelity::koh::KohCalibration`
(with `ThetaEstimate::Posterior`). Differential record:
`verification_and_validation/koh_calibration_gh1180.md`, tests
`tests/koh_sepia_gh1180.rs`. Two upstream facts not to "fix" back: SEPIA's
univariate **setup** cannot carry δ (only its likelihood and dense
predictor have the branch), and its dense `uvPred` scalar + δ branch puts
`SigUUx` in the wrong block (line 704); the port follows upstream's own
GPMSA comment there. TMCMC needs enough moves per stage on this posterior
(3 under-mixed in 11 dimensions; 20 matched SEPIA's MCMC).

### The gh:#1191 FilterPy route (MIT, 2026-11-04)

`src/filtering/` ports FilterPy @ `5df4e095fac0` (2.0.0; `LICENSE-MIT-FILTERPY`,
NOTICE): the linear KF (Joseph-form update, batch filter, RTS smoother),
EKF, UKF with Merwe sigma points, EnKF and the kinematic process-noise
helpers. `numpy_ops.rs` is **not** a port: its `Matrix` kernels reproduce
numpy/OpenBLAS's rounding order, which was **measured**
(`verification_and_validation/filterpy_gh1191/probe_numpy_kernels.py`).
That is what makes the filters bit-identical with FilterPy inside the
measured envelope (dims ≤ 5 for `inv`, ≤ 4 for the Cholesky). Do not
"tidy" an operation order there.
The typed route is `fidelity::StateSpaceModel` with `fidelity::Assimilator`.
Differential record: `verification_and_validation/filterpy_gh1191.md`,
tests `tests/filterpy_gh1191.rs`.

Four upstream facts are ported faithfully and must not be "fixed":
- the UKF re-draws its sigma points after predict (since `94852b4`, which is
  newer than PyPI 1.4.5);
- the EnKF's `P` after an update is `P − K S Kᵀ`, not the ensemble's own
  covariance (`ensemble_covariance()` gives that, beside it);
- the EnKF's `P_xz` uses `self.x`;
- `Q_discrete_white_noise` only takes dim 2–4.

**Synthetic data only:** state estimation is in scope only for twin
experiments (`RESPONSIBLE_USE.md`).

### The gh:#1192 OpenDA route (LGPL-3.0-or-later, 2026-11-04)

`src/openda/` ports OpenDA @ `e51565c2c8de` (`LICENSE-LGPL-3.0-OPENDA`,
NOTICE): the EnKF with its localisation options (none, Hamill, autoZhang),
EnSR, the residual-resampling particle filter, the steady-state filter,
DUD, Simplex, Powell, the weighted cost (`SimulationKwadraticCostFunction`),
`StochVector`, the oscillator / Lorenz test models, and OpenDA's `Matrix`
kernels with **JAMA's SVD (public domain)**, which OpenDA's `inverse`,
`sqrt` and `rightSolve` use. LGPL-3.0-or-later goes into this AGPL crate as
a combined work (GPLv3 / AGPLv3 §13); each ported file keeps OpenDA's LGPL
notice. `DrawSource` is not a port: it replays OpenDA's recorded draws or
uses the workspace LCG (no java.util.Random is reimplemented).
The typed route is `fidelity::Assimilator::OpenDaEnsemble`.
Differential record: `verification_and_validation/openda_gh1192.md`
(references generated by building OpenDA with javac and running
`openda_gh1192/driver/`), tests `tests/openda_gh1192.rs`: every number
bit-identical. Upstream quirks ported faithfully, not to be "fixed" (record
§3): the EnKF's in-place 1/√(q−1) round trip; Zhang's bootstrap members at
**twice** the deviation and `EnsembleVectors` skipping member 0; Simplex's
unsorted first iteration; Powell rescaling its directions in place (and its
Brent stall, A3); DUD's sticky sign flip of `innerScaleFac`; `evaluatePdf`
multiplying by Πσ; the cost dropping the observation at `t_start`.

### Third-party BSD code inside RAVEN

RAVEN's own `NOTICE.txt` discloses vendored third-party code under separate BSD
licences:

- **AMSC** — Copyright 2014 University of Utah, Scientific Computing and
  Imaging Institute (3-clause BSD).
- **NGL** — Copyright 2012 Carlos D. Correa (2-clause BSD).

Both are GPLv3-compatible, but **neither is covered by the Apache-2.0 grant**.
They sit in the topological-decomposition / Morse-Smale area, adjacent to
sensitivity analysis. **Check which upstream component a file comes from before
writing its header**, and use the BSD attribution for those. Read
`NOTICE-RAVEN` for the exact text.

---

## Per-file attribution header — the convention, with a worked example

**Every file in this crate that is derived from an upstream RAVEN file must
carry an attribution header naming the upstream project, the upstream source
file, the version/commit it was taken from, the copyright holder, and the
licence.** Put it at the very top of the file, above the `//!` module doc. Do
not strip it during refactors, do not move it into a separate document, and do
not summarise it away.

Copy this and edit the fields:

```rust
// ---------------------------------------------------------------------------
// Ported from RAVEN (Risk Analysis Virtual ENvironment).
//
//   Upstream project: RAVEN — Idaho National Laboratory
//   Upstream repo:    https://github.com/idaholab/raven
//   Upstream file:    ravenframework/Samplers/MonteCarlo.py
//   Upstream commit:  01216937967c38ee287859270c035c8eca906dc6  (branch devel)
//   Accessed:         2026-08-06
//
//   Copyright 2017 Battelle Energy Alliance, LLC
//   Licensed under the Apache License, Version 2.0 (the "License");
//   you may not use this file except in compliance with the License.
//   You may obtain a copy of the License at
//
//       http://www.apache.org/licenses/LICENSE-2.0
//
//   Unless required by applicable law or agreed to in writing, software
//   distributed under the License is distributed on an "AS IS" BASIS,
//   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//   See the License for the specific language governing permissions and
//   limitations under the License.
//
// This Rust translation is part of RAFFLES / Outram Park and is distributed
// under GPL-3.0-only. Apache-2.0 -> GPLv3 is a ONE-WAY relicensing: this file
// may NOT be contributed back to RAVEN or redistributed under Apache-2.0.
//
// Translation notes: <what changed and why — e.g. RAVEN's Sampler class
// hierarchy is flattened into the `Sampler` enum; NumPy array operations are
// expressed over slices; RAVEN's XML input handling is out of scope.>
// ---------------------------------------------------------------------------
```

Rules for filling it in:

- **`Upstream file`** is a real path in the upstream repo. Verify it exists at
  the commit you name; do not guess a plausible-looking path.
- **`Upstream commit`** is the commit the translation was actually read from.
  If you work from a newer clone, update the commit *and* the `Accessed` date
  in the header, and say so in the crate `NOTICE` if the reference point for
  the whole crate moved.
- **The licence block is Apache-2.0** for files derived from RAVEN proper. For
  a file derived from the vendored **AMSC** or **NGL** code, replace it with
  that component's BSD text and copyright holder instead — see above.
- **`Translation notes` is not optional.** Say what you changed structurally
  and what you deliberately did not port. This is the honest record of how far
  the translation goes.
- **A file written from a published paper rather than from upstream source is
  not a port.** Do not give it this header — cite the paper in the module doc
  comment instead, and say plainly that it is an independent implementation.

---

## Design: RAVEN's inheritance must become Rust enums

This is the single biggest structural decision in the port, and the workspace
rule is hard.

RAVEN is deeply inheritance-based. Its samplers descend through
`Sampler` -> `ForwardSampler` -> `MonteCarlo` / `Grid` / `Stratified` / …, its
distributions through a `Distribution` base into `Uniform`, `Normal`,
`Gamma`, …, and dispatch happens through Python's dynamic method lookup plus a
`Factory` that instantiates classes by name from XML. **None of that maps to
`Box<dyn Trait>` in this workspace.**

Per the root `CLAUDE.md` "Rust design rules":

- **No trait objects** — no `Box<dyn Trait>`, `&dyn Trait`, `Arc<dyn Trait>`
  for dispatch. Each family becomes **one enum with a variant per concrete
  model**, dispatched by `match`.
- **No `Box<T>`** — own by value, or share with `Arc<T>`.
- **No lifetime parameters** on structs, traits or impls. A sampler owns its
  distributions (or shares them via `Arc`); it does not borrow them.

A trait is still useful as a **compiler-enforced contract** on each concrete
struct — the compiler then checks that every distribution really does provide a
CDF and an inverse CDF. It is just not the dispatch mechanism. The pattern:

```rust
/// Compiler-enforced contract every concrete distribution must satisfy.
pub trait ContinuousDistribution {
    /// Probability density at `x`.
    fn pdf(&self, x: f64) -> f64;
    /// Cumulative probability at `x`; result lies in `[0, 1]`.
    fn cdf(&self, x: f64) -> f64;
    /// Inverse CDF (percent-point function); `p` must lie in `[0, 1]`.
    fn ppf(&self, p: f64) -> crate::Result<f64>;
}

/// Dispatches without `Box` or `dyn`.
pub enum Distribution {
    Uniform(UniformDistribution),
    Normal(NormalDistribution),
    LogNormal(LogNormalDistribution),
    // adding a variant here is a compile error at every `match` that forgot it
}

impl Distribution {
    /// Inverse CDF of whichever distribution this is.
    pub fn ppf(&self, p: f64) -> crate::Result<f64> {
        match self {
            Self::Uniform(d)   => d.ppf(p),
            Self::Normal(d)    => d.ppf(p),
            Self::LogNormal(d) => d.ppf(p),
        }
    }
}
```

Why it matters here specifically: adding a distribution or a sampler is exactly
the change most likely to be made later, and the enum makes every site that
forgot to handle it a compile error rather than a silent runtime fallthrough.

Two related translation notes:

- **Drop RAVEN's `Factory` / XML-name-based instantiation entirely.** RAFFLES
  callers construct the enum variant they want in Rust. Input-file parsing is
  out of scope for this crate.
- **Do not reproduce RAVEN's mutable "handler" objects.** Prefer functions that
  take data and return owned results.

---

## Verification: nothing is done without a verification path

**Every ported distribution and every estimator needs a verification path
before it counts as done.** A module that computes numbers nobody has checked
is a draft, and must be described as one.

The reference for each family:

| What | Verify against |
|---|---|
| Continuous distribution | Analytic mean / variance / skewness; CDF and inverse-CDF round trip across the support; published tabulated quantiles |
| Discrete distribution | Exact analytic moments; probability mass summing to 1 over the support |
| Truncated distribution | Renormalisation — mass over the truncated support integrates to 1, and moments match the closed form for the truncated case |
| Monte Carlo sampler | Sample moments converge to the analytic moments at the expected `1/sqrt(N)` rate; a fixed seed reproduces a design bit-for-bit |
| Latin hypercube sampler | Exactly one point per equiprobable stratum per variable; marginals match the requested distributions |
| Grid sampler | Point count and coordinates match the tensor product exactly |
| Sobol indices | The **Ishigami function** — closed-form first-order and total indices at the conventional parameters. Plus an additive linear model (first-order indices sum to 1 and equal the total indices) and the Sobol g-function for a strongly interacting case |
| Correlation measures | A construction with a known correlation matrix; and a monotone non-linear transform of it, where Spearman is preserved and Pearson is not |
| Surrogate | Exact reproduction of a polynomial at the matching expansion order; a published test problem with reported error metrics |
| Fault trees | Upstream **SCRAM built from source and run** on upstream's own input models — cut sets compared set-for-set against SCRAM's products, then totals in all three modes and all five importance factors for every basic event. Fixtures in `reference-data/scram/` |

**Document methodology AND results.** Per the workspace V&V rule, a test whose
docs say only what it does is incomplete. State the reference, the inputs,
sample size, tolerances and pass criterion — and the numbers actually measured,
with their sampling uncertainty and the date they were taken. Never write down
a result that was not produced by running the check.

Note the distinction the workspace draws: these are **verification** gates
("is it implemented correctly?"). Validation ("does it represent reality well
enough?") is a separate question and is not answered by any of the above.

---

## Android / Termux

The crate is Android-clean, and stays that way. Its dependencies are
`thiserror`, `petir` (the RNG — see below; ~~`outram-mc-libs`~~ **CORRECTED
2026-10-02**), `xml-rs` (the MEF reader), `uom` (no_std; the `fidelity` API,
since 2026-11-04, gh:#1194), and the optional `burn`
(see "Machine learning" below); all three build for `aarch64-linux-android` —
`cargo check -p raffles --all-targets --features burn --target
aarch64-linux-android` was clean on 2026-09-16 (burn 0.21.0, rustc 1.94.1).

- **Never** add `ndarray-linalg`, or anything needing system BLAS/LAPACK, a C
  or Fortran toolchain, or windowing GUI, as an unconditional dependency.
- Surrogate fitting and correlated multivariate sampling are where this
  temptation will appear. Reach first for the pure-Rust **`faer`** already in
  the root `[workspace.dependencies]`.
- Examples, tests and benches are **not** exempt — a native Termux build
  compiles them all.

### Follow `outram-mc-libs`'s gating conventions — do not invent a third

If RAFFLES ever needs something Android-hostile, gate it **exactly the way
`outram-mc-libs` already does**, in the same change. Read that crate's
`Cargo.toml` and `src/gpu/` for the worked pattern; do not devise a new one:

- Target-conditional dependency tables —
  `[target.'cfg(not(target_os = "android"))'.dependencies]` (that is how
  `outram-mc-libs` keeps `wgpu` off Android) and the matching
  `…'.dev-dependencies]` for dev-only tools (how it keeps `naga` off).
- `#[cfg(not(target_os = "android"))]` on the items that use them, with a
  CPU-only shim on the Android side so the library still builds headless.
- **Android stub `main`** in any example or binary, under
  `#[cfg(target_os = "android")]`, since a blanked file gives "main function
  not found".

Note the workspace rule while you are here: Android's `target_os` is
**`"android"`, not `"linux"`**, so a `cfg(target_os = "linux")` gate does
**not** mean "not Android" — never rely on one to exclude Android.

### RNG — reuse, do not add or hand-roll one

**RAFFLES must never depend on `outram-mc-libs` (or any crate that depends on
RAFFLES) again (2026-10-02).** It used to, for this generator and for a CSG
graph adapter (`gnn::mc_geometry`). Once `outram-mc-libs` took on
`outram-park-fork-liggghts` for DEM pebble beds, that edge closed the cycle
`outram-mc-libs -> liggghts -> raffles -> outram-mc-libs`. The generator moved
to `petir::rng::lcg`; the adapter moved to `outram_blender::gnn_graph` (feature
`gnn-graph`, GitHub issue #486). Adapters from a physics or geometry crate INTO
RAFFLES' types live in that crate, never here. The reverse edge
`outram-mc-libs -> raffles` is explicitly allowed (maintainer, 2026-10-02:
RAFFLES' statistics can speed up Monte Carlo), which is exactly why RAFFLES
must stay free of physics and geometry dependencies.

**Sampling draws from `petir::rng::lcg`** (OpenMC's 64-bit LCG port;
~~`outram_mc_libs::rng::lcg`~~ **CORRECTED 2026-10-02** — the generator moved
to `petir` unchanged, at the maintainer's direction),
not from a `rand` crate and not from a PRNG written inside RAFFLES. Whether the
workspace should take a general `rand` dependency is an open maintainer
question (`docs/raven-port-scoping.md` section 10, question 1) and is not a
port agent's call. Seeding stays **explicit** — every `generate` takes a
`master_seed`, and independent dimensions/replicates get non-overlapping
streams via `future_seed` jump-ahead. See `src/samplers.rs`.

**Resolved upstream defect — the warning that used to sit here is obsolete.**
`init_seed` (then `outram_mc_libs::rng::lcg::init_seed`) was wrong: it added where OpenMC
multiplies, so consecutive `id`s landed one LCG *step* apart instead of one
*stride*, making per-stream derivation produce near-perfectly correlated
streams. **Fixed in `op-rbo`** — it now matches
`openmc/src/random_lcg.cpp:60`, and is safe to use.

`src/samplers.rs` still calls `future_seed` directly with explicit stride
arithmetic. That is now a *preference*, not a workaround: it makes the stride
widening visible at the call site when a design needs more draws than
`DEFAULT_STRIDE`. Either route is correct today.

**Note the generator's output changed** (`op-jis`): `prn` now applies OpenMC's
PCG-RXS-M-XS output permutation instead of returning raw state bits, which fixed
a measured lattice defect — successive pairs previously occupied 1 of 1024 bins
along the dual-lattice normal. RAFFLES' statistical gates still pass, but any
*recorded* number in `src/samplers.rs` doc comments that was measured before
that change is stale and needs re-measuring before it is cited

Proxy check, all targets:

```bash
cargo check --release -p raffles --all-targets --target aarch64-linux-android
```

The authoritative check is a native build inside Termux.

---

## Machine learning — `burn` replaces PyTorch, and is optional here

Upstream RAVEN backs its machine-learning surrogates with **PyTorch**. The
Rust replacement for PyTorch in this workspace is **`burn`** (tracel-ai,
MIT OR Apache-2.0 — permissive into GPL-3.0, one-way, like RAVEN itself), per
maintainer direction on 2026-09-16: reach for `burn` wherever the upstream
reaches for PyTorch, as far as `burn` will go. Do not add a second ML
framework, and do not hand-roll a tensor/autodiff layer alongside it — the same
reasoning as the RNG rule above.

**It is optional and off by default.** `burn` is declared once in the root
`[workspace.dependencies]` with `default-features = false`, and RAFFLES takes
it as `burn = { workspace = true, optional = true }` behind its own `burn`
feature:

```bash
cargo build -p raffles --release --features burn
```

- **`no_std` + `alloc`.** That is what `default-features = false` buys, and it
  is deliberate: `burn` runs on `core` + `alloc`, which is all a tensor library
  needs, and it keeps the Android/Termux and wasm builds clean. `alloc` is
  implicit in that mode (`burn` declares `extern crate alloc` itself — there is
  no `alloc` feature to name). A `std` binary links a `no_std` `burn` happily,
  so do not turn the defaults back on for convenience; the `std` feature is
  where `burn`'s dataset/network/train/sqlite machinery lives.
- **Backend: `ndarray` only, for now.** The crate's `burn` feature enables
  `burn/ndarray` — the one backend that is pure Rust and libm-backed, with no
  system BLAS and no C/Fortran toolchain. Never swap it for `tch`, `candle`,
  `cuda`, `rocm` or any `blas-*` feature: every one of those violates the
  Android rule above.
- **GPU is out of scope for this first pass** (maintainer direction,
  2026-09-16). When `wgpu`/`vulkan`/`metal`/`webgpu` is wanted, it goes behind
  its own feature under `cfg(not(target_os = "android"))`, following
  `outram-mc-libs`'s gating convention — not into the default build.
- **Three modules consume it:** `surrogate::neural` (a feed-forward regressor),
  `gnn::mpnn` (the message-passing network) and `gnn::training`. Everything
  else in the crate builds without it, which is why it stays off by default.
  `autodiff` is not separable from `ndarray` here — training needs gradients,
  and a `burn` feature that could only run a forward pass would be a trap.
  These are subject to the same verification requirement as everything else —
  a fitted network that reproduces its training data is not a verified
  surrogate.

Verified clean on 2026-09-16 with `burn` 0.21.0 and rustc 1.94.1 (warnings in
the output came from `outram-mc-libs`/`njoy-outram-park-fork` and predate this):

```bash
cargo check --release -p raffles --lib         --features burn
cargo check --release -p raffles --all-targets --features burn --target aarch64-linux-android
cargo check --release -p raffles --lib         --features burn --target wasm32-unknown-unknown
```

---

## Scope boundaries (do not widen without the owner's say-so)

**In scope:** probability distributions, sampling strategies, sensitivity
measures, surrogate models — the statistical core. Plus, since 2026-09-21,
fault-tree quantification in `src/scram/` (see below).

**`src/scram/` was added at the workspace maintainer's direction, not the crate
owner's**, and has since grown to a fairly complete fault-tree analysis layer:
tree construction, cut sets by two routes, prime implicants, a BDD, and the
quantification and importance measures. It widens this crate well past the
RAVEN-derived statistical core it was scoped to.

That is a direction call, and by the ownership rule above it is **Adolphus
Lye's to confirm or reverse**. It is recorded here so it stays visible rather
than absorbed silently.

**An earlier revision of this file said "do not build further on it, or extend
it toward cut-set generation, without checking with them first." That is what
happened anyway**, on the maintainer's explicit instruction to fill in the
port's remaining gaps — the maintainer being the person who set the SCRAM
direction in the first place. The instruction is recorded as superseded rather
than deleted, because the crate owner's review is still outstanding and the
scope has moved a long way since it was written.

**`src/estimators.rs` and `src/distributions/seeded.rs` were added on
2026-10-02 at the workspace maintainer's direction, not the crate owner's**
(GitHub issue #500, under the outram-mc ↔ RAFFLES epic #493). They hold
generic statistics moved out of `outram-mc-libs` — sample mean and standard
error, seed pooling, Shannon entropy over counts, the trigger's running-sum
uncertainty and batch prediction, first-order error propagation — and three
seed-driven samplers (`uniform`, Box-Muller `sample_normal`, `sample_exp`).
The rule they follow: the maths on numbers lives here, and deciding *what* is
counted stays in the physics crate. #493's ownership comment classes a new
basic-estimators module as **needing Adolphus Lye's OK**; the maintainer
directed it to land now, so, as with `scram/`, it is recorded here as a
direction call that is **theirs to confirm or reverse**, with their review
outstanding. (`src/estimators.rs` moved, content unchanged, to
`src/estimators/mod.rs` on 2026-10-03 so the submodules below could sit
beside it.) Until they decide, two constraints bind anyone editing these
files:

- **Bit-identity is the contract.** `outram-mc-libs` pins k-eff results to the
  bit through these functions (`tests/stats_move_fingerprints.rs` there). Do
  not "tidy" an expression — a reassociated sum or a `sqrt(a)/sqrt(b)` in
  place of `sqrt(a/b)` moves recorded results.
- **They are not RAVEN ports** and carry no RAVEN header; each function names
  the outram-mc file (and, through it, the OpenMC routine) it came from.

**Estimator submodules added 2026-10-03 under epic #493, at the workspace
maintainer's direction, not the crate owner's.** #493's ownership comment
classes each of these as **needing Adolphus Lye's OK**; the maintainer directed
them to land now, so they are recorded here like `estimators` itself — a
direction call that is **theirs to confirm or reverse**, with their review
outstanding. They are generic statistics on arrays of numbers; the simulation
drivers that call them (the seed-ensemble runner, the learned weight windows,
the UQ and sweep loops) stay in `outram-mc-libs::stats`, outside this crate's
scope. None of them changes any function that existed before, so the
bit-identity contract above is untouched.

- `src/estimators/seed_consistency.rs` (#494) — `χ²/dof` of seed values
  against their internal `σ` with a fixed two-sided 95 % band, leave-one-out
  Bonferroni outlier flags, disjoint-group-mean scatter (the measured `1/√N`
  check).
- `src/estimators/autocorrelation.rs` (#495) — lag autocorrelations, integrated
  autocorrelation time with Sokal's automatic window (`c = 5`, fixed),
  non-overlapping batch means (default batch `⌊√n⌋`, fixed; leading remainder
  dropped), and `CorrelatedMean`, which always carries the naive `s/√n`
  beside the corrected estimates.
- `src/estimators/stationarity.rs` (#496) — MSER-5 truncation (White 1997),
  Geweke's 10 %/50 % two-window z with batch-means errors (`|z| > 1.96`,
  fixed), and a single change-point in the mean (exact posterior under flat
  mean / Jeffreys variance priors, BIC verdict on the Kass–Raftery scale via
  `model_selection`).

Also added 2026-10-03, but as **ordinary contribution** rather than a scope
change: `src/surrogate/validation.rs` (leave-one-out refits and jackknife+
prediction intervals for `PolynomialSurrogate`; first consumers #497 and
#499). It fills the "cross-validation machinery" that `src/surrogate/mod.rs`
already lists as in scope. The owner has not reviewed it.

`outram-mc-libs` now depends on RAFFLES (allowed edge, maintainer 2026-10-02),
which is exactly why RAFFLES must stay free of physics and geometry
dependencies.

**Out of scope:** physics of any kind; simulation drivers, job scheduling and
run-directory management; databases; RAVEN's optimisers; ~~adaptive /
model-in-the-loop samplers (they need the model-evaluation loop this crate
deliberately does not own).~~ **CORRECTED 2026-11-04** — samplers that call a
caller-supplied likelihood closure are in: `bayesian::transitional` (TMCMC,
TEMCMC) always did, and RAVEN's MCMC samplers (`bayesian::raven_mcmc`,
gh:#1193) do. Still out: RAVEN's adaptive samplers that drive external codes
(limit-surface search, adaptive sparse grids, dynamic event trees).

**`src/fidelity/` was added on 2026-11-04 at the workspace maintainer's
direction, not the crate owner's** (GitHub issue #1194, epic #1177: "I want a
common interface (struct, enum or otherwise) for high fidelity to surrogate
model", then "Integrate uom into the trait interface"). It is recorded here,
like `scram/` and `estimators`, as a direction call that is **Adolphus Lye's to
confirm or reverse**. What it changes about this crate's scope, stated plainly:

- **`fidelity::build` evaluates the caller's model**, through the caller's own
  `HighFidelityModel` implementation, which the sentence below ("the caller
  runs their own model and hands RAFFLES arrays of numbers") did not allow.
  There is still no physics here: the model lives in the caller's crate.
- **Physical quantities cross this API as `uom` types** (traits with
  associated `Input`, `Output`, `Theta: Quantities`; `quantities!` derives the
  mapping). The numerics underneath stay on SI `f64`.
- **Traits are the seam between crates, consumed generically, never `dyn`**
  (root `AGENTS.md`, "Crate-level interfaces may follow SOLID principles");
  the routes RAFFLES provides are the enum `fidelity::Surrogate`.
- **New surrogate routes join as `SurrogateSpec` / `Surrogate` variants when
  their port lands** (kriging and co-kriging with SMT, gh:#1190; polynomial
  chaos with RAVEN, gh:#1193), never as empty placeholder variants.
- **Bit-identity with the underlying route is the contract**: the typed path
  must give the same bits as calling `PolynomialSurrogate` directly (pinned in
  `fidelity::build`'s tests). The held-out split never reaches a fit (pinned
  by a poisoning test). V&V record:
  `verification_and_validation/fidelity_interface_gh1194.md`.

~~input-file / XML parsing; plotting and reporting~~ **CORRECTED 2026-09-22**
— brought into scope for `src/scram/` by the maintainer's "everything except
the GUI" direction. It remains out of scope for the RAVEN-derived modules,
where no such instruction was given.

The caller runs their own model and hands RAFFLES arrays of numbers.

The full port scope — which RAVEN capabilities are in, which are out, and in
what order — lives in the workspace-root **`docs/raven-port-scoping.md`**.

---

## Build and test

```bash
cargo check --release -p raffles --lib
cargo test  -p raffles --lib --tests --release
```

Always `--release` for builds and tests, per the workspace rule.

The optional `burn` feature is **not** in the default build, so add it
explicitly when the work touches the surrogate path:

```bash
cargo check --release -p raffles --lib --features burn
cargo test  -p raffles --lib --tests --release --features burn
```

---

## Intended use

Education, research, capability building and V&V only. Despite the "risk
analysis" name, RAFFLES is **not** for nuclear facility operation, reactor
control, licensing decisions, probabilistic safety assessment of a real
facility, safety-critical decision-making, or emergency response. Do not frame
outputs, examples or docs as authoritative for any of those. See the workspace
`RESPONSIBLE_USE.md`.

## Avoid writing macros (workspace rule, 2026-11-05)

Maintainer direction: macros make code harder to read and review. Do not
write new `macro_rules!` or proc macros in this crate; use functions,
generics, enums and plain `match` arms. Using std and dependency macros
(`vec!`, `format!`, derives) is fine. Full rule: root `CLAUDE.md`, "Avoid
writing macros".
