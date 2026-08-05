# Validation case: MATPRO Zircaloy Poisson's ratio (`PoissonRatioModel::MatproZircaloy`)

**Bead:** `op-6sl.7` · **Crate:** `outram-park-fork-offbeat` ·
**Model under test:** `materials::properties::poisson_ratio::PoissonRatioModel::MatproZircaloy`
· **Companion sources:** [`References.md`](./References.md)

> **V&V stage: Unit Tested → *Verification complete, Validation NOT performed*.**
> This document defines the validation case and reports the one result that
> could be obtained without experimental data. It does **not** validate the
> model. Per the workspace `CLAUDE.md` bookkeeping rule, the crate's
> "Verification & Validation — human-reviewed" axis is the maintainer's personal
> sign-off and is **not** altered by this document.

---

## 1. What is being validated

Poisson's ratio `nu` of Zircaloy cladding as returned by the MATPRO correlation
pair, over 290–1800 K, and as a function of retained cold work, oxygen content
and fast neutron fluence.

MATPRO does not tabulate `nu`. It fits Young's modulus `E` and shear modulus `G`
as **two independent straight lines** and forms

$$ \nu = \frac{E}{2G} - 1 $$

The three-branch structure, transcribed from upstream (see
[`References.md`](./References.md) entry S-1), is:

```text
K1 = (7.07e11 - 2.315e8 * T) * C_ox      oxygen effect        [G]
K2 = -2.6e10 * C_cw                      cold-work effect     [G]
K3 = 0.88 + 0.12 * exp(-phi / 1e25)      fast-fluence effect  [G]

alpha  (T <  1073 K):  G = (4.04e10 - 2.168e7 * T + K1 + K2) / K3
interp (1073-1273 K):  linear in T between the alpha value at 1073 K
                       and the beta value at 1273 K
beta   (T >= 1273 K):  G = 3.49e10 - 1.66e7 * T
```

with the companion `E` sharing the same branch structure and, for fresh
material, the beta-phase line `E = 9.21e10 - 4.05e7 * T`.

Because `E` and `G` are fitted separately, **nothing in the correlation
constrains their ratio.** Poisson's ratio is a derived quantity, and the
derivation amplifies error: writing `r = E/G`, `nu = r/2 - 1`, so
`d(nu) = dr/2`, and near `r = 3` a **1 % error in the ratio `E/G` moves `nu` by
0.015** — 1 % of the entire admissible range `(-1, 0.5)`. This
sensitivity is the reason the model needs validating against a *direct* `nu`
measurement rather than against `E` and `G` separately. (`0.015` is 1 % of the
`1.5`-wide admissible interval `(-1, 0.5)`.)

## 2. Acceptance criteria

Three distinct criteria, because they are answerable independently and only the
first two are currently answerable at all.

### VAL-1 — Thermodynamic admissibility (**no experimental data required**)

For an isotropic linear-elastic solid, positive-definiteness of the
strain-energy density requires

$$ -1 < \nu < 0.5 $$

This is a theorem about the elasticity tensor, not a modelling convention: at
`nu = 0.5` the bulk modulus and the first Lamé parameter
`lambda = E*nu/((1+nu)(1-2nu))` diverge, and above 0.5 `lambda` changes sign.

- **Pass criterion:** `-1 < nu < 0.5` at every state inside the correlation's
  own stated validity range (290–1800 K).
- **Reference:** the physical constraint itself. No dataset is needed, which is
  why this is the one criterion that could be evaluated.
- **Status: EXECUTED — FAILS.** See § 4.

### VAL-2 — Agreement with measured Poisson's ratio (**BLOCKED — no data**)

- **Intended reference data:**
  [`References.md`](./References.md) **E-3** (Schwenk & Wheeler 1978, direct
  `nu` measurement on Zircaloy-4, 297–589 K) as the primary target, and **E-2**
  (Northwood, London & Bähen 1975, Zircaloy-2, 293–773 K) as the secondary.
- **Intended range:** the alpha branch, 293–773 K, which is where cladding
  actually operates and where both datasets live.
- **Proposed pass criterion — NOT YET JUSTIFIED:** a tolerance cannot honestly
  be fixed yet. It must be set from (a) MATPRO's own stated expected standard
  error for CELMOD/CSHEAR ([`References.md`](./References.md) GAP-2) and (b) the
  scatter of the measurements themselves, neither of which was obtainable.
  Setting a number now would be an invented acceptance bar. The tolerance must
  also not be tighter than the elastic anisotropy of textured cladding tubing
  permits any isotropic model to achieve (context: E-8).
- **Status: NOT EXECUTED.** No measured value for Zircaloy `nu`, `E` or `G` was
  obtained in this session; see [`References.md`](./References.md) § "READ THIS
  FIRST" and GAP-3. **No comparison table appears below, because filling one in
  would require inventing the measured column.**

### VAL-3 — Correct trend of `nu` with temperature (**BLOCKED — unverified**)

- **Intended reference:** [`References.md`](./References.md) E-2.
- **Criterion:** the sign of `dnu/dT` predicted by the model must match the
  sign reported by measurement for Zircaloy-2 over 293–773 K.
- **Why it is called out separately:** search-result metadata for E-2 asserts
  that measured Poisson's ratio *decreases* with increasing temperature for
  Zircaloy-2. This port's `nu` *increases* monotonically with temperature
  (Table A). **If that assertion is correct, the model has the wrong sign of
  `dnu/dT` across the entire alpha range** — a defect much broader than the
  `nu > 0.5` crossover, and one that would matter at ordinary operating
  temperature rather than only in a severe-accident transient.
- **Status: NOT EXECUTED, and the premise is UNVERIFIED.** The claim comes from
  a search-engine paraphrase of an abstract that was never read
  ([`References.md`](./References.md) E-2 and GAP-7). It is recorded as the
  highest-priority open question, **not** as a finding. Do not cite it.

## 3. Model output — computed side of the comparison

Every number in this section was **printed by code and transcribed**, never
predicted. Provenance of the numbers themselves:

- **Produced by:** the unmodified, committed
  `src/materials/properties/{poisson_ratio,young_modulus}.rs` of this crate.
- **How:** a throwaway harness in the session scratchpad with a path dependency
  on the crate, `cargo run --release` (release mode per workspace rule).
- **Note on the build:** at the time of the run, concurrent work on
  `src/mechanics/` left the full crate transiently uncompilable. The harness was
  therefore built against a *snapshot copy* of the crate with the
  `mechanics`/`rheology`/`burnup`/`corrosion`/`fgr`/`gap`/`prelude` modules
  commented out of `lib.rs`. The `materials` module has **no** code dependency
  on any of them (it references `mechanics` only in prose), and both property
  files were verified clean against git before copying, so the numbers are those
  of the unmodified property layer.
- **Reproduce with:** any equivalent harness calling
  `PoissonRatioModel::MatproZircaloy.value`,
  `YoungModulusModel::MatproZircaloy.value` and
  `matpro_zircaloy_shear_modulus`. The unit tests in `poisson_ratio.rs` pin the
  key values.

The **"measured"** column that would make these tables a validation comparison
is deliberately absent — see VAL-2.

### Table A — fresh Zircaloy (unirradiated, no cold work, as-received oxygen)

| T [K] | E [Pa] | G [Pa] | nu [-] | branch | admissible |
|---|---|---|---|---|---|
| 290.0000 | 9.292250e10 | 3.411280e10 | 0.361989 | alpha | yes |
| 300.0000 | 9.237500e10 | 3.389600e10 | 0.362624 | alpha | yes |
| 400.0000 | 8.690000e10 | 3.172800e10 | 0.369453 | alpha | yes |
| 500.0000 | 8.142500e10 | 2.956000e10 | 0.377283 | alpha | yes |
| 600.0000 | 7.595000e10 | 2.739200e10 | 0.386354 | alpha | yes |
| 700.0000 | 7.047500e10 | 2.522400e10 | 0.396983 | alpha | yes |
| 800.0000 | 6.500000e10 | 2.305600e10 | 0.409611 | alpha | yes |
| 900.0000 | 5.952500e10 | 2.088800e10 | 0.424861 | alpha | yes |
| 1000.0000 | 5.405000e10 | 1.872000e10 | 0.443643 | alpha | yes |
| 1073.0000 | 5.005325e10 | 1.713736e10 | 0.460355 | interp | yes |
| 1100.0000 | 4.876943e10 | 1.668252e10 | 0.461692 | interp | yes |
| 1200.0000 | 4.401456e10 | 1.499794e10 | 0.467353 | interp | yes |
| 1273.0000 | 4.054350e10 | 1.376820e10 | 0.472360 | beta | yes |
| 1300.0000 | 3.945000e10 | 1.332000e10 | 0.480856 | beta | yes |
| **1354.8387** | 3.722903e10 | 1.240968e10 | **0.500000** | beta | **crossover** |
| 1400.0000 | 3.540000e10 | 1.166000e10 | 0.518010 | beta | **NO** |
| 1500.0000 | 3.135000e10 | 1.000000e10 | 0.567500 | beta | **NO** |
| 1600.0000 | 2.730000e10 | 8.340000e9 | 0.636691 | beta | **NO** |
| 1700.0000 | 2.325000e10 | 6.680000e9 | 0.740269 | beta | **NO** |
| 1800.0000 | 1.920000e10 | 5.020000e9 | **0.912351** | beta | **NO** |

Maximum `nu` from a 200 001-point sweep of the closed interval [290, 1273] K:
**0.472360, attained at the 1273 K endpoint.** The alpha and interpolation
branches therefore stay inside the admissible interval throughout, and the
failure is confined to the beta branch above 1273 K. (`nu` is continuous at
1273 K, so the endpoint value is shared by both branches.)

### Table B — fine sweep across the crossover

`nu = 0.5` is exactly the condition `E = 3G`.

| T [K] | E [Pa] | G [Pa] | E/G [-] | nu [-] |
|---|---|---|---|---|
| 1273.0000 | 4.054350e10 | 1.376820e10 | 2.944720 | 0.472360 |
| 1300.0000 | 3.945000e10 | 1.332000e10 | 2.961712 | 0.480856 |
| 1320.0000 | 3.864000e10 | 1.298800e10 | 2.975054 | 0.487527 |
| 1340.0000 | 3.783000e10 | 1.265600e10 | 2.989096 | 0.494548 |
| 1350.0000 | 3.742500e10 | 1.249000e10 | 2.996397 | 0.498199 |
| **1354.8387** | 3.722903e10 | 1.240968e10 | **3.000000** | **0.500000** |
| 1360.0000 | 3.702000e10 | 1.232400e10 | 3.003895 | 0.501947 |
| 1380.0000 | 3.621000e10 | 1.199200e10 | 3.019513 | 0.509757 |
| 1400.0000 | 3.540000e10 | 1.166000e10 | 3.036021 | 0.518010 |

**Measured crossover, by bisection to 200 iterations:**

```text
crossover T = 1354.838709677 K   (nu there = 0.500000000000)
analytic    = 1.26e10 / 9.3e6 = 1354.838709677 K
```

The analytic value comes from setting `E = 3G` on the beta lines:
`9.21e10 - 4.05e7*T = 3*(3.49e10 - 1.66e7*T)`, i.e. `T = 1.26e10/9.3e6`. Bisection
and algebra agree to all nine printed decimals.

### Table C — retained cold work at 600 K

| cold work [-] | nu [-] | admissible |
|---|---|---|
| 0.0000 | 0.386354 | yes |
| 0.0200 | 0.403506 | yes |
| 0.0500 | 0.430515 | yes |
| 0.0800 | 0.459189 | yes |
| 0.1000 | 0.479308 | yes |
| 0.1197 | 0.499967 | yes |
| 0.1500 | 0.533501 | **NO** |
| 0.2000 | 0.594043 | **NO** |
| 0.3000 | 0.739230 | **NO** |
| 0.5000 | 1.186979 | **NO** |

Bisected threshold at 600 K: **cold work = 0.119730769**.

Mechanism: `K2 = -2.6e10 * C_cw` is subtracted from the numerators of **both**
`E` and `G`. Since `G` is roughly a third of `E`, the same absolute subtraction
costs `G` proportionally three times as much, so the ratio `E/G` — and hence
`nu` — rises with cold work.

### Table D — the cold-work threshold falls sharply with temperature

**This extends the failure well beyond what bead `op-6sl.7` records.** The bead
notes the 600 K threshold of 0.1197; in fact the threshold **decreases
monotonically with temperature across the whole alpha branch**:

| T [K] | cold work at which G = 0 | cold-work threshold for nu = 0.5 |
|---|---|---|
| 300.0 | 1.303692 | 0.179096 |
| 400.0 | 1.220308 | 0.159308 |
| 500.0 | 1.136923 | 0.139519 |
| 600.0 | 1.053538 | 0.119731 |
| 700.0 | 0.970154 | 0.099942 |
| 800.0 | 0.886769 | 0.080154 |
| 900.0 | 0.803385 | 0.060365 |
| 1000.0 | 0.720000 | 0.040577 |
| 1073.0 | 0.659129 | **0.026131** |

At the top of the alpha branch a retained cold-work fraction of only **2.6 %**
is enough to push `nu` past 0.5. Cold-worked stress-relief-annealed (CWSRA)
cladding retains cold work by definition, so this is not an exotic corner of
input space — it is a routine cladding condition.

The second column records where `G` itself reaches zero and turns negative; the
threshold bisection is bracketed strictly below it, because `nu` is monotonic in
cold work only up to that singularity.

### Table E — oxygen content (weight fraction above as-received)

| oxygen content [-] | nu (600 K) | nu (1000 K) |
|---|---|---|
| 0.0000 | 0.386354 | 0.443643 |
| 0.0010 | 0.376349 | 0.440499 |
| 0.0050 | 0.340091 | 0.429340 |
| 0.0100 | 0.301775 | 0.417934 |
| 0.0200 | 0.241993 | 0.400886 |
| 0.0500 | 0.135688 | 0.372632 |

Oxygen moves `nu` **downward**, i.e. away from the admissible bound — it is not
an aggravating factor for this defect. Note the opposite sign of the `T`
coefficient inside `K1` between the `E` and `G` fits (`+par2*T` in `E`,
`-par2*T` in `G`); that asymmetry is upstream's and MATPRO's, and is reproduced
deliberately.

### Table F — fast fluence cancels exactly in the alpha phase

```text
phi = 0.000e0  n/m^2 -> nu = 0.386353679906542
phi = 1.000e24 n/m^2 -> nu = 0.386353679906542
phi = 1.000e25 n/m^2 -> nu = 0.386353679906542
phi = 1.000e26 n/m^2 -> nu = 0.386353679906542
phi = 1.000e27 n/m^2 -> nu = 0.386353679906542
```

`K3` divides both `E` and `G`, so it cancels identically in `E/(2G) - 1` — to
the last printed digit. Fluence therefore cannot mitigate the defect, and the
known upstream `1e4` unit inconsistency between the `E` and `nu` models (see the
module's "Known divergences from upstream") is immaterial to `nu` in the alpha
phase.

### Table G — the constant-Zircaloy alternative

| T [K] | MATPRO nu | ConstantZircaloy nu | difference |
|---|---|---|---|
| 300 | 0.362624 | 0.300000 | 0.062624 |
| 600 | 0.386354 | 0.300000 | 0.086354 |
| 1000 | 0.443643 | 0.300000 | 0.143643 |
| 1400 | 0.518010 | 0.300000 | 0.218010 |
| 1800 | 0.912351 | 0.300000 | 0.612351 |

### Table H — room-temperature anchors

```text
T = 293.15 K: E = 9.275004e10 Pa, G = 3.404451e10 Pa, nu = 0.362188
T = 297.15 K: E = 9.253104e10 Pa, G = 3.395779e10 Pa, nu = 0.362442
T = 300.00 K: E = 9.237500e10 Pa, G = 3.389600e10 Pa, nu = 0.362624
```

297.15 K (24 °C) is the low end of the E-3 measurement range, tabulated here so
that the comparison is a subtraction once E-3 is obtained.

## 4. Result

**VAL-1: FAIL.** The model returns a thermodynamically inadmissible Poisson's
ratio over a substantial part of its own stated validity range.

- Crossover of `nu = 0.5` for fresh Zircaloy at **T = 1354.838709677 K**
  (bisection, 200 iterations; analytic `1.26e10/9.3e6` agrees to nine decimals).
- `nu` reaches **0.912351 at 1800 K**, the top of upstream's stated range —
  i.e. **445.16 K of the 1510 K validity interval — 29.5 % of it — lies beyond
  the crossover.**
- A second, independent failure regime exists in cold work, and it reaches down
  to ordinary operating temperature: threshold **0.179096 at 300 K**, falling to
  **0.026131 at 1073 K** (Table D).
- Fast fluence cancels exactly and cannot mitigate it (Table F); oxygen content
  moves `nu` the safe way (Table E).

**VAL-2 and VAL-3: NOT EXECUTED** — no measured Zircaloy elastic data was
obtainable in this session. See [`References.md`](./References.md) GAP-3, GAP-4
and GAP-7.

**Date of these results:** 2026-08-05, against the working tree of branch
`claude/outram-foam-8ookor`.

## 5. Interpretation

**This is a faithful port of a real upstream defect, not a porting bug.** The
coefficients and branch structure were compared character-by-character against
upstream `PoissonRatioMatproZy.C`
([`References.md`](./References.md) S-1); the port reproduces them exactly, and
upstream neither detects nor guards the condition.

The root cause is structural. MATPRO fits `E` and `G` as independent straight
lines in temperature. Two straight lines with different slopes have a ratio that
varies monotonically and without bound, so `E/G` inevitably crosses 3 somewhere.
For the beta-phase fits it crosses at 1354.84 K — comfortably inside the range
MATPRO claims. No choice of the `K1`/`K2`/`K3` correction factors can prevent
this, because they do not act on the beta branch at all: `G_beta = par9 -
par10*T` ignores oxygen, cold work and fluence entirely.

The consequence for a mechanics solve is not inaccuracy but nonsense:

$$ \lambda = \frac{E \nu}{(1 + \nu)(1 - 2\nu)} $$

is singular at `nu = 0.5` and sign-flipped above it, so the elasticity tensor
stops being positive definite. `mechanics::LinearElastic::new` rejects such
constants, so a hot-Zircaloy case fails outright here where upstream would
silently produce a garbage stiffness. **The port's behaviour is the better of
the two**, but "fails at assembly time with an unclear error" is not the same as
"handled".

### Upstream behaviour — relevant to the gating decision

Upstream's own default Zircaloy material **does not use this model**.
`offbeatLib/materials/materialModel/zircaloy.C`, lines 90–94:

```cpp
    YoungModulus_ =
    YoungModulusModel::New(mesh, materialModelDict, "ZyMATPRO");

    PoissonRatio_ =
    PoissonRatioModel::New(mesh, materialModelDict, "ZyConstant");
```

The Young's modulus defaults to the MATPRO fit; **Poisson's ratio defaults to
`ZyConstant`**, the constant 0.3. `PoissonRatioMatproZy` is opt-in only, and
upstream's user manual lists `constantPoissonRatioZy.H` as the default model for
Zircaloy. Whether that was a deliberate avoidance of this defect or incidental
is not documented — but it means selecting the MATPRO Poisson model is already
a departure from upstream's own recommended configuration.

## 6. Limitations of this validation case

1. **It is not a validation.** VAL-1 tests the model against a physical
   *constraint*; only VAL-2/VAL-3 test it against *reality*, and those did not
   run. A model can satisfy VAL-1 everywhere and still be wrong.
2. **No measured value appears anywhere in this document or in
   [`References.md`](./References.md).** This is deliberate. Full text of every
   cited work was unreachable, and a plausible-looking invented data point in a
   validation case would silently corrupt every future comparison.
3. **The traceability of the coefficients stops at OFFBEAT.** Upstream cites
   only "MATPROv11" with no equation numbers, no references and no uncertainty.
   Until [`References.md`](./References.md) GAP-1 is closed, the coefficients are
   attributable to OFFBEAT, not to MATPRO.
4. **Isotropy is assumed.** Textured cladding tubing is elastically anisotropic,
   so a single scalar `nu` is already an approximation independent of everything
   above (context: E-8).
5. **Upstream composition differs.** Upstream divides the MATPRO shear modulus
   by whatever Young's-modulus *field* is on the mesh registry, which need not
   be MATPRO's. This port pairs the MATPRO `G` with the MATPRO `E` — the
   internally consistent combination. A user who paired MATPRO `G` with a
   constant `E` upstream would get a different crossover, not no crossover.

## 7. Recommended next actions

1. Obtain [`References.md`](./References.md) **R-4** (NUREG/CR-7024) — most
   likely single source for the equations, the fit uncertainty and a
   model-to-data comparison. Closes GAP-1 and GAP-2.
2. Obtain **E-3** (Schwenk & Wheeler 1978) and digitise its `nu(T)` for
   Zircaloy-4, documenting the digitisation steps per `DATA_POLICY.md`. Closes
   GAP-3 and makes VAL-2 executable.
3. Read **E-2** to settle GAP-7 — the `dnu/dT` sign question, which is
   potentially a larger defect than the one this document reports.
4. Set the VAL-2 tolerance from (1) and (2), never before.
5. Decide the gating policy — § 8. **Maintainer's decision; not implemented.**

## 8. Gating policy — options and recommendation

`PoissonRatioModel::is_admissible()` exists precisely so the mechanics layer can
gate on this. The policy itself is a **maintainer decision and is deliberately
not implemented** by this document.

| Option | Behaviour | Assessment |
|---|---|---|
| **A. Clamp** `nu` just below 0.5 | Solve proceeds with `nu = 0.5 - eps` | **Not recommended.** `lambda` is arbitrarily large near 0.5, so the answer is dominated by the clamp epsilon, not the physics — an unstable, silently wrong stiffness. It also fabricates a number the correlation never produced, and hides the failure from the user. |
| **B. Fall back to `ConstantZircaloy`** (0.3) | Substitute the constant when `nu >= 0.5` | **Not recommended as a silent fallback.** It has upstream precedent (§ 5) and stays admissible, but Table G shows the discontinuity at the switchover is 0.218 at 1400 K — a jump in `nu` mid-solve, which would show up as a spurious stress transient. If chosen, it must be a **case-setup-time** substitution that is logged, never a per-cell runtime swap. |
| **C. Refuse the case** with a clear error | Reject at setup when the case's temperature/cold-work envelope crosses the admissible limit | **Recommended.** |

**Recommendation: C, refuse — with the diagnostic doing the real work, and B
available as an explicit opt-in.**

Reasoning:

1. **The model has no valid answer to give here.** Past the crossover the
   correlation is not inaccurate, it is inadmissible: the elasticity tensor is
   not positive definite. Clamping or substituting invents a value; refusing
   reports the truth. This is what `VERIFICATION_AND_VALIDATION.md` means by
   preferring correctness over plausible output.
2. **The failure is already loud, just unclear.** `LinearElastic::new` rejects
   the constants today, so a hot case fails regardless. The improvement
   available is not *whether* it fails but *whether the user learns why*. A
   diagnostic naming the model, the temperature (or cold work), the computed
   `nu`, the crossover at 1354.84 K, and the `ConstantZircaloy` alternative
   converts an opaque assembly failure into a one-line fix.
3. **Refusing at setup beats refusing mid-solve.** The envelope is knowable
   before the first timestep. Checking the case's temperature and cold-work
   range up front — rather than per cell, per timestep — turns a
   thousand-timesteps-in crash into an immediate rejection, and costs nothing in
   the inner loop.
4. **Cold work makes silent handling actively dangerous.** Table D: at 1073 K a
   2.6 % retained cold-work fraction is enough. A user running CWSRA cladding at
   ordinary temperature could trip this without ever going near 1355 K. Under
   option A or a silent option B they would get a plausible-looking answer from
   a model that had quietly stopped being the model they selected. That is the
   worst outcome of the three.
5. **B stays available, explicitly.** A user who genuinely needs a beta-phase
   mechanics solve should select `ConstantZircaloy` deliberately — which is what
   upstream's own default already does (§ 5) — and wear the known inaccuracy
   knowingly. That is a different act from the code choosing it for them.

Given `RESPONSIBLE_USE.md`'s scope (education, research, V&V — explicitly not
safety-critical or licensing analysis), a hard refusal costs a user a re-run
with a documented flag. A silent clamp costs them a wrong answer they cannot
see, in a code whose entire purpose is verification and validation.

**Whatever is chosen, it must be recorded here and in the module docs, and the
gate must never be allowed to make the defect invisible.**
