# Cross-check: `raffles::bayesian` against Adolphus Lye's own MATLAB

**Date:** 2026-09-16.
**Status:** verification only. Nothing here is validation, and nothing here has
been reviewed by the crate owner or the workspace maintainer.

## Why this document exists

`crates/raffles/src/bayesian/` was written **from the published papers**, not
from Adolphus Lye's source, because at the time the licence position on six of
his seven repositories was unclear (see `NOTICE`, and
`docs/adolphus-uq-port-scoping.md`). Writing from a paper is legitimate
provenance, but it leaves a specific hole: **the implementation had never been
compared against the author's own.**

A paper states an algorithm; an implementation settles the dozen choices the
paper leaves open. Those choices are where a re-implementation silently
diverges. This document closes that hole by running the author's MATLAB and
comparing.

It is the same standard the workspace applies elsewhere —
`outram-park-fork-liggghts` against LIGGGHTS-PUBLIC, `petir` against GSL,
`outram-park-fork-dwsim-libs` against DWSIM: **build the upstream, run it,
compare numbers**, rather than reading it and asserting agreement.

## What was compared, and how

| | |
|---|---|
| Upstream | [`Adolphus8/transitional_ensemble_mcmc`](https://github.com/Adolphus8/transitional_ensemble_mcmc) |
| Commit | `eca0338` (shallow clone, 2026-09-16) |
| Licence | GPL-3.0 by direct grant from the copyright holder — see `NOTICE` |
| Files read | `TEMCMCsampler.m`, `TMCMCsampler.m`, `EMCMCsampler.m` |
| Runner | GNU Octave 8.4.0 with `octave-statistics` 1.6.3 |

### The upstream was patched, and here is every patch

MATLAB-only plumbing does not run under Octave. **Three lines** were changed
across the two sampler files, all of them argument handling. No line of
algorithm was touched, and the patched copies are diffable against the clone.

| File | Original | Patched to | Why |
|---|---|---|---|
| both | `internal.stats.parseArgs(pnames, dflts, varargin{:})` | `parseArgsShim(...)` | `internal.stats.*` is a MATLAB-private namespace. The shim does name/value matching against `pnames` with defaults `dflts` and returns them in order — nothing else. |
| both | `if ~isempty(gcp('nocreate'))` | `if false` | `gcp` probes a MATLAB parallel pool. The block only prints a core count. |
| `TEMCMCsampler.m` | `isoctave = (exist('OCTAVE_VERSION','builtin') > 0)` | `isoctave = false` | The author's own Octave branch uses `p = p.addParamValue(...)`, which modern Octave's handle-class `inputParser` rejects ("function called with too many outputs"). The MATLAB `addParameter` branch works under Octave 8. **This is a real bug in the upstream's Octave support**, not in its algorithm. |
| `TMCMCsampler.m` | `'logpdf', log_posterior` into `mhsample` | `'pdf', @(t) exp(log_posterior(t))` | **A bug in the runner, not in either code** — see below. Safe for this problem only, where the posterior density stays around 1e-8 and does not underflow. |

### The one that nearly became a false accusation

The first TMCMC run **stalled**: `beta` crept from 0.0187 to 0.0299 over 28
stages in increments of 3e-5 and was never going to reach 1. Read naively,
that is a damning result about the upstream sampler.

It is not. It is a defect in **octave-statistics 1.6.3's `mhsample`**, which
`TMCMCsampler.m` calls. Probed directly on a `N(3, 1)` target with a symmetric
Gaussian proposal, 20 000 draws, thin 3, burn-in 1 000:

| call | recovered mean | recovered sd | acceptance |
|---|---|---|---|
| `'logpdf', @(x) -0.5*(x-3).^2` | **-211.77** | **66.93** | **1.0000** |
| `'pdf', @(x) exp(-0.5*(x-3).^2)` | 2.9788 | 0.9902 | 0.7862 |
| `'pdf'` + `'symmetric', true` | 2.9788 | 0.9902 | 0.7862 |
| target | 3.0000 | 1.0000 | — |

The `'logpdf'` path **accepts every proposal** — acceptance is exactly 1 — so
it performs an unrejected random walk and diffuses away. The `'pdf'` path is
correct. `TMCMCsampler.m` uses `'logpdf'`, which is the right choice for real
work and simply is not implemented correctly by this package.

Two things follow, and both matter more than the TMCMC numbers themselves:

- **The upstream is exonerated.** Nothing about the stall is attributable to
  `TMCMCsampler.m`; under MATLAB, where `mhsample` works, it would not occur.
  Had this been reported without the probe it would have been a false
  accusation against a collaborator's code, which is exactly the failure mode
  the workspace's "read upstream first / check before you claim" rules exist
  to prevent. **A cross-code result is only as trustworthy as the runner, and
  the runner has to be checked too.**
- **`TEMCMCsampler.m` never touches `mhsample`** — it uses the vendored
  ensemble sampler — so the TEMCMC comparison above is unaffected by this and
  needed no workaround.

## Findings

Eleven differences were found. F1, F2 and F11 are checked numerically by tests
in `src/bayesian/transitional.rs`; the rest are structural and are recorded
here. Summarising where each one lands:

| | |
|---|---|
| **No defect in this crate's mathematics** | none of the eleven |
| A gap in this crate's *completeness* | **F3** (no adaptive proposal scaling) |
| This crate is the more correct of the two | **F7** (detailed balance), **F8** (prior stays in the target), **F10** (numerical stabilisation) |
| A defect in the **upstream**, diagnosed and confirmed | **F11** (the prior cancels out of the MH ratio for a non-uniform prior) |
| Convention or default, no right answer | F1, F4, F5, F6, F9 |

F11 is the one worth reading; it is the thing this exercise was for.

### F1 — Tempering solver: population vs sample standard deviation ✅ tested

Ching and Chen's criterion is `CoV(w) = 1`. The paper does not say which
estimator of the standard deviation. MATLAB's `std` defaults to the **sample**
form (`N - 1`); this crate's `weight_cov` uses the **population** form (`N`).

Octave confirms his solver lands exactly where the *sample* CoV is 1 and the
*population* CoV is `sqrt(199/200) = 0.99749686716299`.

Consequence: **this crate's tempering steps are 0.507 % larger**, measured
identically at two different likelihood spreads. Re-solving this crate's
bisection at his target reproduces his `d_beta` to **1.4e-14 relative**, which
establishes that the convention is the *entire* difference — there is no second
discrepancy hiding underneath.

Kept as-is deliberately: the exact identity `ESS = N / (1 + CoV²)`, which is
what makes Ching and Chen's `CoV = 1` and Lye and Marino's `ESS = N/2` the same
criterion, holds only for the population form.

Test: `tempering_solver_matches_the_matlab_up_to_the_std_convention`.

### F2 — Evidence increment: agrees to floating-point noise ✅ tested

`S(j) = mean_i exp(d_beta · lnL_i)`, the quantity that accumulates into
`ln Z`. Worst disagreement over five cases: **8.9e-16 absolute (2-4 ulp)**.
The estimator is cross-code verified against its author's implementation.

Test: `evidence_increment_matches_the_matlab`.

### F3 — No adaptive proposal scaling ❗ genuine gap in this crate

Both of his samplers steer the acceptance rate toward `0.23 + 0.21/D` between
stages. This crate does neither — `scale` and `a` are fixed for the whole run.

```matlab
% TMCMCsampler.m:  beta starts at 2.4/sqrt(D)
c_a  = (acceptance - ((0.21./Dimensions) + 0.23))./sqrt(j);   % note the 1/sqrt(j) damping
beta = beta .* exp(c_a);

% TEMCMCsampler.m: stepsize starts at 2, no damping, floored at 1.01
c_a      = (acceptance_rate - ((0.21./Dimensions) + 0.23));
stepsize = stepsize.*exp(c_a);
if stepsize <= 1, stepsize = 1.01; end
```

This is the most substantive finding. Without it a badly-chosen scale stays
badly chosen for every stage, and the sampler has no way to recover. **Whether
to implement it is the crate owner's call** — it changes sampler behaviour and
would invalidate every recorded V&V number in the module.

### F4 — Metropolis-Hastings proposal scale default: 0.2 vs 2.4/√D

This crate defaults to `scale = 0.2`, citing Ching and Chen. His TMCMC starts
at `2.4/sqrt(D)` — the Gelman optimal-scaling rule — which is **12× larger in
one dimension**. Both are defensible published choices for the same algorithm,
and with F3 implemented the starting value matters much less. Recorded because
a reader comparing the two would otherwise think one of them is a typo.

### F5 — Resampling: systematic vs multinomial

His: `randsample(N, N, true, wj_norm)` — multinomial. This crate: systematic
(one uniform, `N` equally-spaced strata). Both unbiased; systematic has
strictly lower variance at the same cost. A difference in Monte Carlo noise,
not in the target distribution.

### F6 — Moves per stage: 1 vs 3

His thins both samplers by 3 (`'thin', 3` into `mhsample`; `thinchain`
default 3). This crate's `chain_length` defaults to 1. **This is a default, not
a capability gap** — set `chain_length = 3` to match. His chains therefore
decorrelate roughly three times as much per stage, at three times the
likelihood cost.

### F7 — Ensemble move: complementary halves vs single population

This crate splits the ensemble in two and moves each half against the other —
the Foreman-Mackey (2013) parallel scheme, which preserves detailed balance.
His `EMCMCsampler` updates one population in place, and draws **a single random
partner offset shared by every walker in a sweep**:

```matlab
rix = mod((1:Nwalkers)+floor(rand*(Nwalkers-1)),Nwalkers)+1;
```

Updating walkers in place against partners that have already moved this sweep
breaks the detailed-balance argument Goodman and Weare give. In practice with
2000 walkers the effect is small, and it is inherited from the upstream
`gwmcmc` (Grinsted 2015) that his file vendors. **This crate is the more
correct of the two here**; the difference is noted so nobody "fixes" the split
to match.

### F8 — Out-of-support proposals: reject the move vs redraw the proposal

His redraws in a `while true` loop until the proposal is inside the prior's
support, so the proposal is a *truncated* density — and the truncation's
normalising constant is omitted from the Hastings ratio, which biases the
kernel when the truncation bites unevenly. This crate rejects the move instead,
which is exactly correct and needs no correction term. **This crate is the more
correct of the two here** as well.

For an unbounded prior, as in the conjugate case below, the two coincide.

### F9 — Non-finite log-likelihood: error out vs treat as impossible

His: `if any(isinf(log_fD_T_thetaj)), error('The prior distribution is too far
from the true region'); end`. This crate maps a non-finite or NaN
log-likelihood to `-inf` and carries on, so a prior with support outside the
model's domain is survivable rather than fatal. A deliberate divergence; his
behaviour is the louder one and arguably better for a first-time user, which is
worth revisiting.

### F10 — Numerical stabilisation

He forms `wj = exp((pj1-pj)*log_fD_T_thetaj)` raw. This crate shifts by the
maximum log-weight and adds it back. Algebraically identical (F2 measures how
identical), but the raw form underflows to all-zero weights for a sharply
peaked likelihood, taking `S(j)` to 0 and `ln Z` to `-inf`. **This crate is
strictly more robust**; no change wanted.

## End-to-end comparison

Both implementations run on the same conjugate Normal-Normal problem the Rust
tests already use — 12 observations, known `sigma = 1`, prior `mu ~ N(0, 5²)` —
where the posterior mean, posterior standard deviation and log-evidence are all
available in closed form. `N = 2000`, ten seeds each.

Closed form: mean **2.491694**, sd **0.288195**, `ln Z` **-15.685402**.

### TEMCMC — `N = 2000`, 5 seeds each

His at his own defaults (`stepsize = 2`, `thinchain = 3`, no burn-in); ours at
the crate defaults, and again at `chain_length = 3` to match his thinning.
Each cell is the mean over seeds ± the seed-to-seed spread.

| | posterior mean | posterior sd | `ln Z` | stages |
|---|---|---|---|---|
| closed form | 2.491694 | 0.288195 | -15.685402 | — |
| **his** TEMCMC | 2.493997 ± 0.010172 | 0.287737 ± 0.006247 | -15.686675 ± 0.030611 | 3.2 |
| ours, `chain_length = 1` | 2.492526 ± 0.006797 | 0.288598 ± 0.005196 | -15.677201 ± 0.058985 | 3.2 |
| ours, `chain_length = 3` | 2.491482 ± 0.005911 | 0.286966 ± 0.003604 | -15.663922 ± 0.043585 | 3.2 |

Separation between the two implementations, in standard errors of the
difference:

| | posterior mean | posterior sd | `ln Z` |
|---|---|---|---|
| ours at `chain_length = 1` | 0.27 | 0.24 | 0.32 |
| ours at `chain_length = 3` | 0.48 | 0.24 | 0.96 |

**They agree.** Every separation is under one standard error against a 3-sigma
criterion, and both bracket the closed form. The average stage count is 3.2 on
both sides, so F1's 0.507 % step-size difference does not even change how many
stages a run takes here.

Test: `temcmc_ensemble_matches_the_matlab_ensemble`.

#### One suspicion retired

The single-seed number recorded on
`temcmc_matches_the_conjugate_normal_posterior` has a posterior sd 2.4 % above
the closed form, and the obvious reading — given F6 — was that
`chain_length = 1` under-decorrelates next to his thinning of 3.

**It does not.** Over 5 seeds, `chain_length = 1` gives 0.288598 against the
closed-form 0.288195, **+0.14 %**, with a seed spread of ±0.005 that comfortably
covers the 2.4 % seen in the single run. The earlier figure was seed noise.

This is worth recording as a lesson rather than a footnote: **a V&V number taken
from one seeded run is worth much less than it looks**, and the module's
single-run numbers should be read with that spread in mind. Re-recording all of
them as seed ensembles is the obvious follow-up.

### TMCMC — `N = 500`, 20 seeds each

His at his own defaults; ours at the crate defaults. The smaller budget is
because his TMCMC drives 500 separate `mhsample` chains per stage under Octave.

| | posterior mean | posterior sd | `ln Z` | stages |
|---|---|---|---|---|
| closed form | 2.491694 | 0.288195 | -15.685402 | — |
| **his** TMCMC | 2.507103 ± 0.016021 | 0.289952 ± 0.008963 | -15.735060 ± 0.076958 | 3.5 |
| ours | 2.488174 ± 0.017216 | 0.289459 ± 0.015316 | -15.697938 ± 0.139207 | 3.4 |

Separation, in standard errors of the difference: posterior sd **0.12**,
`ln Z` **1.04**, posterior mean **3.60**.

Test: `tmcmc_ensemble_matches_the_matlab_ensemble`.

### F11 — the posterior mean does not agree, and here is why ❗ upstream defect

The posterior mean is 3.60 standard errors apart. Against the closed form,
**this crate sits 0.91 SE low (consistent) and his sits 4.30 SE high**. The
discrepancy is on his side.

`TMCMCsampler.m`'s `prop_pdf` multiplies the Gaussian proposal density by
`box(x)`, and `box` is the **full prior PDF**, not a support indicator:

```matlab
function proppdf = prop_pdf(x, mu, covmat, box)
% Box function is the Prior PDF in the feasible region.
% So if a point is out of bounds, this function will return 0.
proppdf = mvnpdf(x, mu, covmat).*box(x);   % q(x,y) = q(x|y)
```

Write `q(x'|x) = N(x'; x, Σ)·prior(x')`. The Metropolis-Hastings ratio is then

```
 prior(x')·L(x')^β · N(x; x', Σ)·prior(x)      L(x')^β
------------------------------------------  =  -------
 prior(x)·L(x)^β  · N(x'; x, Σ)·prior(x')      L(x)^β
```

— **the prior cancels**. The chain targets the tempered *likelihood*, not the
tempered posterior. The prior survives only in the initial draw and the
importance weights, which is why the effect is a modest pull toward the MLE
rather than an outright wrong answer. Here the likelihood mode is the data mean
`ȳ = 2.5` and the posterior mean is 2.491694, shrunk toward the prior mean 0;
his 2.507103 sits on the far side of 2.5, in the predicted direction.

#### The control run, with the prediction stated first

Predicted before measuring: **with a uniform prior the bias must vanish**,
because a constant density cancels regardless, and `box` then does exactly the
support-indicator job its own comment describes. If the uniform case were also
biased, this diagnosis would be wrong.

His TMCMC, unchanged, same data, `mu ~ U(-20, 20)`, 20 seeds:

| | measured | closed form | separation |
|---|---|---|---|
| posterior mean | 2.499298 ± 0.009651 | 2.500000 | **0.33 SE** |
| posterior sd | 0.288969 ± 0.008080 | 0.288675 | 0.16 SE |
| `ln Z` | -16.680954 ± 0.099535 | -16.719657 | 1.74 SE |

Confirmed: 4.30 SE → 0.33 SE on the one quantity affected, from changing
nothing but the prior.

#### Scope of the defect, stated fairly

- **It requires a non-uniform prior.** Every tutorial and example in his
  repositories uses a uniform prior, so it has had no opportunity to appear
  there, and no published result of his is implicated by this.
- **`TEMCMCsampler.m` is unaffected.** It uses `box` only as a boolean
  rejection test (`if box(proposedm(i,:)), break; end`), never as a density.
  That is consistent with the TEMCMC comparison agreeing to under one standard
  error on all three quantities.
- **It is the same helper used two ways**: the boolean use is correct, the
  density use is not.
- **This crate does not have the equivalent problem.** It rejects
  out-of-support moves rather than folding the prior into the proposal, so the
  prior stays in the target where it belongs (finding F8).

**This is for the author, not for this repository to patch.** Nothing in
`raffles` changes as a result. It is recorded here because it is the single
most useful thing this cross-check produced, and because it is exactly what a
cross-check is for: the Rust and the MATLAB were each individually plausible,
and only running both on a case with a known answer separated them.


## What this does NOT establish

- **It is not validation.** Both codes agreeing on a conjugate Gaussian says
  they implement the same algorithm correctly; it says nothing about whether
  transitional MCMC is the right tool for any particular model-updating
  problem.
- **It does not cover the whole module.** `src/distance.rs`, `src/abc.rs`,
  `src/imprecise.rs` and `src/model_selection.rs` correspond to
  `Approximate_Bayesian_Computation`, `stochastic-model-updating` and
  `Computing-with-Confidence` — two of which are R, not MATLAB — and none has
  been cross-checked. That is the obvious next piece of work.
- **The samplers were not compared trajectory-by-trajectory**, only in
  distribution. The two use different RNGs, different resampling and different
  proposal kernels, so a bit-level comparison of the kind
  `outram-park-fork-liggghts` achieves against LIGGGHTS is not available here
  and would not be meaningful.
