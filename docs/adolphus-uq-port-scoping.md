# Scoping: Adolphus Lye's UQ repositories → `raffles`

Bayesian model updating and imprecise probability, ported into `crates/raffles`
from seven repositories authored by **Adolphus Lye**.

- **Tracked as:** beads epic **`op-dwqw`** (8 children), GitHub issue **#158**
- **Owner:** Adolphus Lye. `raffles` is his crate; direction, scope and priority
  for this work are his to set.
- **Filed:** 2026-09-09

## Why this is separate from the RAVEN port

`raffles` already has an epic, **`op-vjw`**, for porting RAVEN's UQ core. This
work is a **sibling of it, not a part of it**, and the two must not be merged:

| | `op-vjw` (RAVEN) | `op-dwqw` (this) |
|---|---|---|
| Upstream | idaholab/raven | Adolphus8/* (seven repos) |
| Upstream licence | Apache-2.0 | GPL-3.0 (by grant; see below) |
| Direction | Apache → GPL is **one-way** | author is the crate owner |
| Covers | samplers, distributions, sensitivity, surrogates | Bayesian model updating, imprecise probability |

They have different upstreams, licences and `NOTICE` obligations, so keeping the
provenance chains separate is what makes either of them auditable.

**RAVEN does not cover Bayesian model updating**, which is the gap this epic
fills, and the reason issue #158 exists: updating a digital twin against plant
data and a high-fidelity simulation is a model-updating problem, not a
forward-UQ one.

## The seven repositories

Licences as reported by the GitHub API on 2026-09-09.

| Repository | Language | LICENSE on GitHub | Size | Disposition |
|---|---|---|---|---|
| `Transitional_Ensemble_MCMC` | MATLAB | **none** | 207 KB | port — `op-dwqw.3` |
| `Approximate_Bayesian_Computation` | MATLAB | **none** | 10.8 MB | port method only — `op-dwqw.4` |
| `stochastic-model-updating` | R | **none** | 171 KB | port — `op-dwqw.5` |
| `Computing-with-Confidence` | R | **none** | 154 KB | port — `op-dwqw.6` |
| `Bayesian-Model-Updating-Tutorials` | MATLAB | **GPL-3.0** | 4.5 MB | examples — `op-dwqw.7` |
| `Lecture_Resources` | MATLAB | **none** | 33 MB | triage — `op-dwqw.8` |
| `Project_PROMAP` | MATLAB | **none** | 5.8 MB | triage — `op-dwqw.8` |

## Licence provenance — read this before porting anything

**Adolphus Lye is the copyright holder of all seven repositories and has granted
GPL-3.0.** He offered either Apache-2.0 or GPL-3.0; GPL-3.0 is assumed, stated
to the maintainer on **2026-09-09**. He is a collaborator on this repository.
GPL-3.0 is `raffles`' own licence, so there is no compatibility question and no
one-way constraint of the kind that applies to the RAVEN port.

**The gap to close, tracked as `op-dwqw.1`:** six of the seven repositories carry
**no LICENSE file**. Absent one, the public default is all-rights-reserved, so a
third party reading this GPL-3.0 repository **cannot verify from the sources
themselves** that the ported material was licensed — they would have to take an
assertion in a commit message on trust.

That is a provenance gap rather than a legal one, and
`RESEARCH_INTEGRITY_AND_PROVENANCE.md` expects licence and attribution to be
*checkable by a reader*. Two things fix it, and neither blocks porting:

1. Record the grant in `crates/raffles/NOTICE` — per repo: URL, commit ported
   from, the grant, its date, and that it came directly from the copyright holder.
2. Ask Adolphus to add a `LICENSE` file to the six. One commit each, and the
   grant becomes self-evident at the source.

`Bayesian-Model-Updating-Tutorials` already carries GPL-3.0 and needs neither.

## What `raffles` has today, and what is missing

Measured 2026-09-09, not estimated: **5,705 lines across 5 files, 34 tests, zero
`todo!()` or `unimplemented!()`**.

**Present** — `distributions` (Uniform, Normal, LogNormal, Triangular,
Exponential, Weibull, Gamma, Beta, Truncated, behind a `Distribution` enum),
`samplers` (MonteCarlo, LatinHypercube, GridSampler, seeded streams),
`sensitivity`, `surrogate`.

**Absent** — anything Bayesian. A search for `mcmc|bayes|metropolis|likelihood`
across `src/` matches only prose in `lib.rs`. There is no likelihood, no
prior/posterior, and no MCMC of any kind. So this epic is new capability, not a
re-wrap.

> Note: several crate-level docs still describe `raffles` as "scaffold only,
> nothing implemented". That is stale — see bead **`op-blw`**.

## Ordering

`op-dwqw.2` (likelihood / prior / posterior) is the foundation and **blocks the
four sampler ports**, which all plug into the same interface; `op-dwqw.7`
(tutorials) additionally waits on TMCMC, since a tutorial needs a sampler to
demonstrate. `op-dwqw.1` (provenance) and `op-dwqw.8` (triage) are independent
and can start immediately.

Get `op-dwqw.2`'s interface reviewed before building on it. Four ports landing on
a wrong abstraction is the expensive failure mode here.

## Constraints carried from the workspace rules

- **Reuse the existing `Distribution` enum for priors.** Do not introduce a
  parallel distribution type — that is exactly the silent-drift duplication the
  search-first rule exists to prevent.
- **Enum dispatch**; no `Box<dyn>`, `&dyn`, or lifetime parameters.
- **Cite the upstream file and commit in the doc comment** of every ported item,
  so the Rust and the MATLAB/R cannot drift unnoticed.
- **V&V needs methodology *and* measured results.** Recover a known posterior
  analytically wherever one exists.
- **Imprecise probability is a different object** from a precise distribution.
  Expect `op-dwqw.6` to need a new type rather than an enum variant, and raise
  the design before building it.

## Data policy

Issue #158 motivates this as "updating live DT with plant data". Per
`DATA_POLICY.md` and `RESPONSIBLE_USE.md`, only open-source, public-literature,
or properly-licensed public benchmark data may be used anywhere in this project —
**operational facility data is prohibited**, and digital-twin work here is
offline demonstration only, never connected to live or safety-critical systems.

Develop and validate this work against public benchmarks and synthetic cases.
The *methods* are the deliverable; real plant data is out of scope for this
repository regardless of what the methods would eventually be applied to
elsewhere.

## Not yet done

- **Nothing has been ported, and no repository has been cloned or read** beyond
  the GitHub API metadata in the table above. Languages, sizes and licences are
  measured; the mathematical content of each repository is **not** yet reviewed.
- Whether `Lecture_Resources` or `Project_PROMAP` contain library-worthy methods
  is genuinely unknown — that is what `op-dwqw.8` is for.
- No human V&V is claimed for anything this epic produces.
