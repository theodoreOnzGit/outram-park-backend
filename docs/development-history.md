# Development history — neutronics k-eigenvalue path

A running, dated record of the iterative development behind OUTRAM PARK's Monte
Carlo criticality path. Kept at paper quality: each entry states what was tried,
the quantitative result (with uncertainty), the diagnosis, and the adjustment
made. The intent is that this file can be lifted, largely as-is, into the
methodology / development section of a publication — the value of a benchmark
result is inseparable from the path taken to it.

> Scope: the `outram-mc-libs` Monte Carlo transport kernel and the
> `njoy-outram-park-fork` nuclear-data toolkit it pulls cross sections from. For
> the forward-looking plan see [`keff-doppler-roadmap.md`](keff-doppler-roadmap.md);
> for the data-tier design see [`data-acquisition.md`](data-acquisition.md).

---

## 2026-07 — First end-to-end Godiva k_eff, and the fast-spectrum lesson

### 1. First cut: Watt-spectrum-weighted fast MGXS

The two-tier data strategy (see [`data-acquisition.md`](data-acquisition.md))
splits each nuclide's cross section at a per-nuclide ceiling energy `e_max`:

- **Below `e_max`** — windowed multipole (WMP), continuous-energy, with analytic
  Doppler broadening. This is WMP's home turf (thermal + resonance).
- **Above `e_max`** — a coarse fast multigroup set (MGXS), because the WMP
  ceilings sit *far* below the fission spectrum (U-235 `e_max` ≈ 2.25 keV, while a
  bare-sphere fission spectrum lives at 0.1–10 MeV).

Group cross sections are only valid for the weighting spectrum they were
collapsed with. For a fast critical assembly the physically-motivated first
choice is the **Watt fission spectrum**

$$\chi(E) \;\propto\; e^{-E/a}\,\sinh\!\sqrt{bE},$$

with the U-235 thermal-fission parameters `a = 0.988 MeV`, `b = 2.249 MeV⁻¹`.
The fast range is smooth, so group-averaging is accurate there — this is
multigroup's home turf, and Godiva/Jezebel k_eff was historically nailed to a few
hundred pcm with 16–26 group sets. So the fast MGXS for the CORE nuclides was
baked with a single Watt weight, and the transport kernel was wired end-to-end:
WMP + fast MGXS + ν̄ pulled through the CE↔MG seam at `e_max`, isotropic-CM
elastic scatter, analog fission banking, and a homogeneous-sphere power
iteration.

### 2. Result: Godiva runs, but k_eff is high

`cargo run --release -p outram-mc-libs --example godiva_keff` on the ICSBEP
**HEU-MET-FAST-001 (Godiva)** bare U(93.7) sphere (r ≈ 8.741 cm, benchmark
k_eff = 1.0000 ± 0.0010) converged, stable, to:

| Quantity | Value |
|---|---|
| Rust MC k_eff | **1.12852 ± 0.00174** |
| Reactivity bias vs benchmark | **+12 852 ± 174 pcm** |

The result is reproducible and statistically converged — the ±174 pcm is the
Monte Carlo standard error, small relative to the ~12 850 pcm bias, so the
discrepancy is a genuine modelling bias, not noise. That distinction matters: a
converged-but-biased result points at the physics, not the sampling.

### 3. Diagnosis: the modelled spectrum is too hard

A ~13 000 pcm over-prediction on a bare fast HEU sphere is the classic signature
of a **spectrum that is too hard** (too many neutrons kept at high energy, where
ν̄ and the fission-to-absorption ratio are most favourable). Two first-cut
approximations both push the same way:

1. **Infinite-dilution fast MGXS (no self-shielding).** The fast group constants
   were collapsed at infinite dilution, so the resonance-region absorption in the
   unresolved range just above `e_max` is under-counted — fewer neutrons are
   removed, k rises.
2. **Inelastic and (n,xn) lumped into elastic scatter.** With no explicit
   inelastic energy-loss law, neutrons that should have been down-scattered by
   inelastic collisions instead scatter quasi-elastically off a heavy nucleus and
   lose almost no energy. The spectrum stays artificially hard.

Both are known, expected shortcomings of a *first* cut — documented as such in
[`keff-doppler-roadmap.md`](keff-doppler-roadmap.md) — not a defect in the WMP or
transport machinery, which reproduce U-238 Doppler broadening faithfully
(6.673 eV capture resonance: 22 262 b at 0 K → 4 283 b at 1000 K).

### 4. Adjustments

Two changes followed directly from the diagnosis. Neither is claimed to close the
full gap on its own; together they give the levers and the reference data needed
to drive the bias down in subsequent iterations.

**(a) Selectable group-collapse weighting spectrum.** The single hardcoded Watt
weight became an explicit enum,
`njoy_outram_park_fork::nuclear_data::WeightingSpectrum`, so the fast MGXS can be
re-baked under different assumptions and the k_eff sensitivity measured directly:

- `Watt { a, b }` — fission spectrum (default; the physically-correct weight for a
  fast assembly).
- `OneOverE` — 1/E slowing-down flux, the right weight across the resonance /
  epithermal range of a moderated system.
- `Maxwellian { temp_ev }` — a thermal-peak weight, `E·e^{-E/kT}`.

Exposing the weight makes the spectrum assumption an explicit, auditable input
rather than a buried constant — and makes "how much does the weighting spectrum
move Godiva?" a one-line experiment (`cargo run --example bake_mgxs -- <nuclide>
<e_max> <weight>`).

**(b) HIGH-fidelity ENDF reference path wired.** To have an on-device reference to
gate the LOW tier against, the HIGH tier (`--features net-fetch`) now downloads
raw ENDF tapes from a pinned upstream (`njoy_outram_park_fork::acquire`,
IAEA NDS `download-endf`, selectable via the `EndfLibrary` enum) and runs the
crate's own RECONR for fully resonance-reconstructed pointwise σ(E). This is the
authoritative curve against which the multigroup collapse — and the
self-shielding treatment that comes next — is judged. Verified end-to-end on
U-235 (ENDF/B-VII.1, Reich-Moore LRF=3): thermal fission ~970 b at 0.01 eV
(1/v), ~1.3 b in the fast range.

### 5. Open items (next iteration)

The physics fixes that should actually close the ~12 850 pcm gap, in priority
order:

1. **Fast self-shielding** — Bondarenko f-factors or probability tables in the
   unresolved range just above `e_max` (U-238 ~20–150 keV especially), replacing
   the infinite-dilution group averages.
2. **A real inelastic / (n,xn) energy-loss law**, so down-scatter out of the fast
   group is modelled instead of lumped into elastic.
3. **Weighting-spectrum sensitivity study** using lever (a) above, to quantify and
   report the k_eff dependence on the collapse spectrum.

---

## 2026-07 — LOW vs HIGH fidelity: isolating the bias

The next iteration answered the question the first one raised: *is the ~12 850 pcm
Godiva overprediction caused by the coarse LOW-tier data, or by the transport
physics?* The HIGH tier makes this a controlled experiment — swap the
cross-section source and change nothing else.

### Methodology

Identical Godiva model (bare HEU sphere, r = 8.7407 cm, ICSBEP HEU-MET-FAST-001
atom densities) and identical power iteration (5000 histories × [40 inactive +
110 active]) were run under two data tiers, judged against the benchmark
k_eff = 1.0000 ± 0.0010:

- **LOW** (`godiva_keff`, offline) — embedded WMP below `e_max` + infinite-dilution
  Watt-collapsed 10-group fast MGXS above.
- **HIGH** (`godiva_keff_endf`, `net-fetch`) — the same three isotopes downloaded
  as raw **ENDF/B-VII.1** tapes and reconstructed on device:
  RECONR (0.1% tol) → BROADR (Doppler to 293.6 K) → energy-dependent ν̄ from
  MF=1/452. Continuous-energy pointwise σ(E) throughout, so resonance
  self-shielding is captured implicitly (σ sampled at each neutron's actual
  energy) — the single biggest fidelity lever available short of new physics.

ENDF/B-VII.1 (not VIII.0) is used because its U resonances are Reich-Moore
(LRF=3), which the RECONR port reconstructs; VIII.0 U is LRF=7 (not yet ported).
On-device reconstruction of all three isotopes takes ~14 s.

### Results (2026-07, ENDF/B-VII.1)

| Data tier | k_eff | Δk vs benchmark |
|---|---|---|
| LOW (embedded WMP + fast MGXS) | 1.12852 ± 0.00174 | +12 852 pcm |
| HIGH (ENDF CE, reconstructed) | 1.12451 ± 0.00202 | +12 451 pcm |
| **Effect of the data upgrade** | **−0.00401** | **≈ −400 pcm** |

### Finding

Replacing coarse, infinite-dilution group data with full continuous-energy
resonance-reconstructed data — the largest data-fidelity improvement in the
pipeline — moves k_eff by only **~400 pcm**, barely 3% of the ~12 500 pcm bias.
The overprediction is therefore **transport-physics-limited, not data-limited.**

The bias lives in the approximations *shared* by both runs: inelastic and (n,xn)
scattering lumped into an elastic-like event (no real energy-loss law), and
isotropic-CM elastic scatter. Both keep the neutron spectrum too hard — too many
neutrons stay at high energy, where ν̄ and the fission/absorption ratio are most
favourable — regardless of how accurate the cross sections are. This reorders the
Open-items priority from the previous entry: **the inelastic energy-loss law and
anisotropic scatter, not fast self-shielding, are the levers that will close the
Godiva gap.** Fast self-shielding remains correct to add, but the comparison bounds
its k_eff worth at a few hundred pcm.

This result (data alone barely moves k_eff) is one half of the HIGH-tier Godiva
test `outram-mc-libs::physics::keff::tests::godiva_high_fidelity_reaches_benchmark`
(behind the `net-fetch` feature); the other half — that the *transport* physics
added in the following entries closes the gap — is what makes the HIGH tier reach
the benchmark.

---

## 2026-07 — Inelastic scattering: acting on the transport-limited diagnosis

The LOW-vs-HIGH comparison above bounded the data-fidelity worth at ~400 pcm and
pointed at the transport physics — specifically the absence of an inelastic
energy-loss law — as the dominant remaining bias. This entry acts on that
diagnosis and measures the result.

### Methodology

An explicit inelastic scattering channel was added to the transport kernel, drawing
on the level structure the HIGH tier already carries (RECONR reconstructs every
MT=51…91 section, each with its MF=3 QI Q-value). The physics:

- **Discrete levels (MT=51…90)** — two-body CM kinematics generalised from the
  existing elastic formula to a non-zero Q-value:
  `E_cm = E·(A/(A+1))² + Q·A/(A+1)`, isotropic in CM, transformed to the lab
  (`outram_mc_libs::physics::scatter::two_body_scatter`; elastic is the `Q = 0` case).
  Each collision removes the level excitation energy |Q| — tens of keV to over an
  MeV — the large per-collision loss that elastic scatter off A≈238 cannot provide
  (elastic α = ((A−1)/(A+1))² ≈ 0.98).
- **Continuum (MT=91)** — a Weisskopf evaporation secondary-energy spectrum
  `f(E') ∝ E'·exp(−E'/θ)`, nuclear temperature `θ = √(E/a)`, level-density
  parameter `a ≈ A/11 MeV⁻¹` (`continuum_inelastic_scatter`). RECONR gives the
  cross section (MF=3) but not the MF=5 secondary law, so this is an explicit,
  documented approximation.

The collision partition in `transport_history` gained an inelastic bucket between
absorption and elastic (`fission | capture | inelastic | elastic`), sampled per
collision proportional to the summed MT=51…91 σ at the neutron's energy and
dispatched to the matching kinematics. Everything else — geometry, data, power
iteration — is unchanged. The channel is active only for the HIGH (`Pointwise`)
tier, which carries the resolved levels; the LOW tier reports zero inelastic and is
unaffected (see the open item below).

Same Godiva model and settings as the example run (5000 histories ×
[40 inactive + 110 active]), ENDF/B-VII.1, judged against HEU-MET-FAST-001.

### Results (2026-07, ENDF/B-VII.1)

| HIGH-tier transport | k_eff | Δk vs benchmark |
|---|---|---|
| Elastic-only (inelastic lumped into elastic) | 1.12451 ± 0.00202 | +12 451 pcm |
| **+ explicit inelastic energy-loss law** | **1.09942 ± 0.00169** | **+9 942 pcm** |
| **Effect of modelling inelastic** | **−0.02509** | **≈ −2 510 pcm** |

### Finding

Adding one transport-physics channel moved k_eff **~2 510 pcm** — **six times** the
~400 pcm the entire continuous-energy data upgrade delivered — confirming the
previous entry's diagnosis directly: the fast-spectrum bias was transport-limited,
and inelastic down-scatter was the single largest missing lever. The spectrum is
now materially softer: neutrons that previously stayed near 2 MeV (where ν̄ and the
fission/absorption ratio are most favourable) are down-scattered by hundreds of keV
per inelastic collision, removing reactivity.

~9 900 pcm of overprediction remains. The next levers, in expected order: (1)
**anisotropic elastic scatter** (ENDF MF=4 a₁ — forward-peaked elastic changes the
leakage of a bare sphere); (2) **the same inelastic law for the LOW tier**, so the
embedded-data path is not left with the too-hard spectrum this entry just fixed for
HIGH (the group total already contains inelastic — it can be carved out as
`total − elastic − fission − capture` with no data re-bake, using the evaporation
energy-loss law since group data carries no per-level Q); (3) fast self-shielding.

Until the LOW tier also models inelastic, the LOW-vs-HIGH comparison no longer
isolates *data* fidelity alone — HIGH now additionally carries the resolved level
structure LOW lacks — so the regression assertion in the HIGH-tier Godiva test was
widened accordingly.

---

## 2026-07 — Anisotropic elastic scatter: closing the Godiva gap

The previous two entries left ~9 900 pcm on the table and named **anisotropic
elastic scatter** the next lever: fast neutrons scatter forward off heavy nuclei,
and modelling that as isotropic-in-CM understates the transport cross section
`σ_tr = σ_s(1 − μ̄)`, shortening the diffusion length and suppressing leakage from a
bare sphere. This entry adds it and measures the result.

### Methodology

Rather than reinvent the sampling, the angular path was **ported from OpenMC**
(the crate's cited C++ reference at `../openmc`): `AngleDistribution::sample`
(`src/distribution_angle.cpp`) and `Tabular::sample_unbiased` (`src/distribution.cpp`,
lin-lin branch). The elastic angular distribution itself comes from the njoy fork's
existing MF=4 parser (`ace::angular::parse_elastic_angular`), which converts ENDF
MF=4/MT=2 (Legendre or tabulated, LTT=1/2/3) into ACE-form CM-frame tabulated
cosine/pdf/cdf per incident energy. The transport chain:

- `Nuclide::from_endf` now also parses MF=4/MT=2 from the same downloaded tape and
  stores the `ElasticAngular` alongside the reconstructed σ(E) (HIGH tier only).
- `Nuclide::sample_elastic_mu_cm(E)` locates the incident-energy bin, picks the
  lower/upper tabulated distribution by the OpenMC statistical-interpolation rule
  (`r > ξ`), and inverts the cosine CDF (lin-lin quadratic inverse).
- The elastic branch in `transport_history` samples `μ_cm` from that distribution
  when present and feeds it to `two_body_scatter_with_mu` (the elastic kinematics
  generalised to a supplied CM cosine); it falls back to isotropic-CM when absent
  (LOW tier, or an ENDF-isotropic nuclide/energy).

Cosines are in the CM frame (ENDF LCT=2), matching the two-body kinematics. Same
Godiva model and settings (5000 histories × [40 inactive + 110 active]),
ENDF/B-VII.1, judged against HEU-MET-FAST-001 (k_eff = 1.0000 ± 0.0010).

### Results (2026-07, ENDF/B-VII.1)

| HIGH-tier transport | k_eff | Δk vs benchmark |
|---|---|---|
| Isotropic-CM elastic (with inelastic) | 1.09942 ± 0.00169 | +9 942 pcm |
| **+ anisotropic (MF=4) elastic** | **0.99627 ± 0.00175** | **−373 pcm** |
| **Effect of elastic anisotropy** | **−0.10315** | **≈ −10 300 pcm** |

Full journey, HIGH tier: 1.12451 (CE data, elastic-only) → 1.09942 (+inelastic) →
**0.99627 (+anisotropic elastic)**.

### Finding

Elastic anisotropy is by far the largest single lever — **~10 300 pcm** — and it
brings Godiva into agreement with the benchmark: **k_eff = 0.99627 ± 0.00175, i.e.
−373 pcm**, within ~2σ of the MC uncertainty of unity. This is the expected
behaviour for a bare fast-metal assembly: with scattering the dominant fast
interaction and leakage the dominant loss, the isotropic approximation had been
retaining reactivity that a correct forward-peaked transport cross section lets
escape.

Two honest caveats. (1) Landing within a few hundred pcm almost certainly involves
some **cancellation** of the residual approximations still present — no fast
self-shielding, and the Weisskopf-evaporation stand-in for the true MF=5 continuum
secondary-energy law — so the excellent agreement should not be read as each
sub-model being individually exact. (2) The result is HIGH-tier only; the LOW
(embedded) tier still carries neither inelastic nor anisotropic elastic and remains
at ~+12 800 pcm.

Ranked contribution to closing the ~12 500 pcm original bias, HIGH tier:
anisotropic elastic (~10 300 pcm) ≫ inelastic scatter (~2 500 pcm) ≫
continuous-energy data (~400 pcm). The ordering is the durable lesson: for fast
bare-metal criticality, **the transport angular/energy-transfer physics dominates
the cross-section-data fidelity** by more than an order of magnitude.

This is encoded as a benchmark assertion — the HIGH tier must land near unity and
far closer than LOW — in
`outram-mc-libs::physics::keff::tests::godiva_high_fidelity_reaches_benchmark`
(behind the `net-fetch` feature).

## 2026-07 — Porting the two levers down to the LOW (embedded, offline) tier

The HIGH-tier study above closed the Godiva gap with two mechanisms — inelastic
down-scatter and forward-peaked elastic — but both rode on continuous-energy data
(resolved MT=51…91 levels; the full MF=4 angular shape). The question this entry
answers: **can the same two levers be carried by the embedded LOW tier, whose fast
range is only 10-group Watt-collapsed data with no network and no on-device
reconstruction?** The motivation is that the offline tier is what most users run;
leaving it ~12 800 pcm hot while HIGH reaches the benchmark is a large,
avoidable fidelity cliff.

### Methodology

Both levers were reduced to what a group model can carry, then baked into the
embedded MGXS (the MGXL blob bumped **v1 → v2** to add one column):

- **Inelastic (no new data).** The group `total` already includes inelastic, so
  the LOW tier carves it out as the remainder `σ_t − σ_el − σ_f − σ_γ` (clamped ≥ 0)
  in `Nuclide::xs_at_energy`. There are no per-level Q-values in group data, so
  `sample_inelastic` returns `Continuum` for the LOW tier and the existing
  Weisskopf-evaporation law (`continuum_inelastic_scatter`) supplies the energy
  loss — the same law HIGH uses for MT=91.
- **Forward elastic (one number per group).** The bake step
  (`bake_mgxs` → `Mgxs::collapse_from_reconr`) now parses MF=4/MT=2 with the njoy
  fork's existing `parse_elastic_angular`, computes the pointwise mean cosine
  μ̄(E) (`ElasticAngular::mean_cosine`, the P1 moment ∫μ f dμ), and collapses it to
  a **per-group μ̄** on a *scattering-rate* weight σ_el(E)·φ(E) — the average that
  conserves the transport cross section. The result is one μ̄ per group (U-238:
  0.033 → 0.88 across 20 keV → 20 MeV), stored in the MGXS.
- **Sampling μ̄ back into an angle.** The naive linearly-anisotropic (P1) law
  `½(1 + 3μ̄μ)` is invalid once `μ̄ > 1/3` — and for Godiva a large share of the
  flux sits in the 0.3–5 MeV groups where μ̄ is 0.3–0.8. So the LOW tier samples
  the **maximum-entropy exponential law** `p(μ) ∝ exp(λμ)` instead, whose mean is
  the Langevin function `L(λ) = coth λ − 1/λ`; `sample_exponential_mu` solves
  `L(λ) = μ̄` (Newton, `langevin_inverse`) and inverts the CDF. It reproduces any
  `μ̄ ∈ (−1, 1)` and stays a valid density everywhere — verified by unit tests that
  recover the target mean to < 5×10⁻³ at μ̄ = 0.6 and 0.85.

Same Godiva model and settings as every prior entry (5000 histories × [40 inactive
+ 110 active]); LOW tier uses embedded ENDF/B-VIII.0 group data; judged against
HEU-MET-FAST-001 (k_eff = 1.0000 ± 0.0010).

### Results (2026-07, ENDF/B-VIII.0 group data, LOW tier)

| LOW-tier model | k_eff | Δk vs benchmark |
|---|---|---|
| elastic-only, isotropic-CM (before) | 1.12852 ± 0.00174 | +12 852 pcm |
| **+ inelastic (evaporation) + forward elastic (μ̄)** | **1.01022 ± 0.00177** | **+1 022 pcm** |
| **Combined effect** | **−0.11830** | **≈ −11 800 pcm** |

### Finding

The embedded, offline tier reaches essentially the same place as the
network-reconstructed HIGH tier — **1.01022 ± 0.00177 (+1 022 pcm)** vs HIGH's
0.99627 (−373 pcm) — from coarse 10-group data plus a **single per-group mean
cosine**. This confirms the durable lesson from the HIGH study in a stronger form:
the two dominant levers are *transport-physics* mechanisms (energy transfer and
forward peaking), not data-fidelity ones, so they transfer down to group data with
almost their full effect. It also validates the maximum-entropy exponential angular
law as an adequate one-parameter stand-in for the full MF=4 shape in the fast range.

Honest caveats, symmetric with HIGH: the residual +1 022 pcm rides on remaining
LOW-tier approximations (no self-shielding; one μ̄ instead of the full angular
shape; evaporation instead of resolved inelastic levels; infinite-dilution group
constants), and near-agreement likely involves some cancellation among them — not
each sub-model being individually exact. The LOW result is ~1 400 pcm *above* HIGH,
consistent with the group model retaining a little more reactivity than
continuous-energy transport.

Encoded in the `godiva_keff` example's V&V doc block and exercised offline by
`outram-mc-libs::physics::keff::tests::godiva_converges_to_sane_keff` (LOW tier), with
the exponential sampler / Langevin inverse covered by unit tests in
`material::nuclide::tests`. Data format: MGXL **v2** (`from_blob` still reads v1,
zero-filling μ̄).

## 2026-07 — (n,2n) multiplicity: restoring the yield-2 neutron (HIGH tier)

### Methodology

Before this change the reaction partition in `physics::keff::transport_history`
was `fission | capture | inelastic | elastic`, and the comment was explicit that
the elastic bucket "sweeps up any residual (n,xn) as an elastic-like event" — i.e.
an (n,2n) collision was transported as a *single* down-scatter, silently dropping
the second neutron. On a bare fast metal sphere that is a real (if small) neutron
multiplier being discarded, biasing k low.

The fix is a faithful port of OpenMC's `inelastic_scatter` (`src/physics.cpp:1167`):
evaluate the reaction's neutron yield and, for an integral yield *Y > 0*, create
*Y − 1* secondary neutrons at the primary's post-scatter energy and direction (the
incident one continues). For (n,2n), *Y = 2* → one extra neutron.

Concretely:
- **Data.** `MicroXS` gains an `n2n` field. The HIGH (`Pointwise`) tier reads it
  from the reconstructed **MF=3/MT=16** background (`recon.eval_mt(Mt16N2n, e)`) —
  a threshold reaction with no resonance contribution, so it needs no
  reconstruction, only to be carried through. The LOW tier reports `n2n = 0`
  (no group column yet — a pending bake).
- **Transport.** `transport_history` now drives a **same-generation work stack**
  (mirroring OpenMC's `create_secondary` bank): the source neutron plus any (n,2n)
  secondaries are tracked to completion *within the current generation*; only
  *fission* neutrons bank to the next generation. The partition gains an `(n,2n)`
  band between inelastic and elastic; when hit, the incident neutron down-scatters
  (Weisskopf-evaporation stand-in — we have no parsed MF=6 (n,2n) emission law) and
  one secondary is pushed at the same outgoing state.

Reference: OpenMC C++ at `../openmc/`, per the new outram-mc-libs porting rule
(mirror the canonical source; scaffold only what is genuinely absent).

### Results (2026-07-03, ENDF/B-VII.1, 5000 particles / 40 inactive + 120 active)

Same-settings, same-seed A/B on the Godiva HIGH tier:

| (n,2n) | HIGH k_eff | vs benchmark |
|---|---|---|
| forced off | 0.99701 ± 0.00168 | −299 pcm |
| **on (yield 2)** | **0.99872 ± 0.00173** | **−128 pcm** |

Worth = **+171 ± 241 pcm** — correct sign (an extra neutron raises k) but only
~0.7σ, **not statistically resolved from zero** at this statistics. LOW is
1.01024 in both runs (unaffected, `n2n = 0`), confirming reproducibility.

### Finding

The measured (n,2n) worth for Godiva is tens of pcm — below the MC noise floor of
a practical run — exactly as the physics predicts: U (n,2n) has a ~5–6 MeV
threshold and samples only the thin high-energy tail of the fission spectrum. So
this is a **fidelity/correctness** change, not a lever that measurably moves
Godiva's k. Its value shows up for (n,xn)-sensitive spectra (harder sources,
Be/D-reflected systems). The HIGH-tier residual (−128 pcm) is now dominated by the
remaining emission-side approximation — a **fixed thermal-Watt χ instead of the
energy-dependent ENDF MF=5 fission spectrum** — tracked as a separate TODO.

Exercised by `physics::keff::tests::godiva_high_fidelity_reaches_benchmark`
(net-fetch). LOW-tier (n,2n) awaits an MT=16 group column in the MGXS bake.

## 2026-07 — Energy-dependent fission spectrum χ (ENDF MF=5, HIGH tier)

### Methodology

The final emission-side approximation flagged by the (n,2n) work was the fission
*birth* spectrum: every fission neutron was born from a single fixed thermal-Watt
χ (`a = 0.988 MeV`, `b = 2.249 MeV⁻¹`), energy-independent and not even per-nuclide.
This change replaces it with the real **ENDF MF=5 / MT=18** prompt fission neutron
spectrum χ(E→E'), where the outgoing-energy distribution depends on the incident
energy of the neutron that induced the fission.

- **Parser (njoy).** `FissionSpectrum::from_endf_mf5` reads MF=5/MT=18 for the
  **LF=1** ("arbitrary tabulated secondary energy distribution") law — a TAB2 over
  NE incident energies, each an inner TAB1 g(E→E'). ENDF/B-VII.1 U-234/235/238 are
  all LF=1, NK=1, p(E)≡1. It builds a per-incident CDF by integrating each density
  (lin-lin trapezoids / histogram bins) and renormalising so `cdf[last] = 1`,
  matching what ACER precomputes for OpenMC. `NK ≠ 1` or `LF ≠ 1` (the LF=5/7/9/11
  evaporation/Maxwell/Watt-with-E-dependent-parameters laws) returns `None` → the
  caller keeps the Watt stand-in.
- **Storage.** `Nuclide` gains a `chi: FissionSpectrum`. The HIGH (`from_endf`) tier
  parses MF=5; the LOW (`from_core`) tier keeps the Watt default (no embedded MF=5).
- **Sampling (openmc).** `Nuclide::sample_fission_energy(e_in, seed)` samples the
  tabulated χ via `sample_continuous_tabular`, a direct port of OpenMC
  `ContinuousTabular::sample` (`src/distribution_energy.cpp`): locate the
  incident-energy bin + factor `r`, statistically pick the lower/upper table
  (`r > ξ`), invert its outgoing-energy CDF (lin-lin quadratic / histogram linear),
  then scale between the neighbouring tables' [E₁, E_K] envelopes. Specialized to
  `n_discrete = 0` (a fission spectrum has no discrete lines) and a lin-lin incident
  grid (ENDF INT=2). The k-eigenvalue driver now births fission neutrons from the
  *fissioning nuclide's* χ at the collision energy `e`, replacing the global Watt.

Reference: OpenMC C++ at `../openmc/`, per the outram-mc-libs porting rule.

### Results (2026-07-03, ENDF/B-VII.1, 5000 particles / 40 inactive + 120 active)

Paired A/B on the Godiva HIGH tier — same reconstruction, same seed, χ toggled:

| Fission χ | HIGH k_eff | vs benchmark |
|---|---|---|
| fixed thermal Watt | 0.99872 ± 0.00173 | −128 pcm |
| **energy-dependent MF=5** | **1.00367 ± 0.00182** | **+367 pcm** |

Worth = **+495 ± 251 pcm** — positive and ~2.0σ, **marginally resolved** (more so
than (n,2n)). LOW is 1.01024 in both, bit-identical (Watt default unchanged),
confirming the change is isolated to the HIGH birth spectrum. Parser sanity check
against the cached U-235 tape: NE = 20 incident energies (1e-5 → 2e7 eV), each
outgoing table normalized (cdf[last] = 1.000000), mean outgoing energy ~2.03 MeV.

### Finding

The U-235 MF=5 mean outgoing energy (~2.03 MeV) is close to the thermal-Watt mean,
so the worth comes from the **shape, not the mean**: the tabulated χ keeps a larger
fraction of births in the productive 1–3 MeV band (above the U-238 fast-fission
threshold, where ν̄ is higher) and fewer in the leaky high-energy tail that the Watt
form over-populates — hence the positive shift on a leakage-dominated bare sphere.
This is the last of the three emission/transport fidelity fixes; the HIGH tier now
carries continuous-energy data, anisotropic elastic, an inelastic energy-loss law,
(n,2n) multiplicity, and a real fission birth spectrum, landing at +367 pcm.

Exercised by `physics::keff::tests::godiva_high_fidelity_reaches_benchmark` and the
`godiva_keff_endf` example (both net-fetch), and by
`material::nuclide::tests::{ct_table_uniform_reproduces_mean,
continuous_tabular_hardens_with_incident_energy}` (offline sampler unit tests).
Remaining TODOs: a parsed MF=6 (n,2n) emission law (Weisskopf stand-in today),
and MF=5 χ for the LOW tier. (LF=7/9/11 + NK>1 mixtures ported below, 2026-07-03.)

## 2026-07 — Remaining MF=5 laws: LF=7/9/11 and NK>1 mixtures (njoy)

### Methodology

The MF=5 work above only ported **LF=1** (arbitrary tabulated), the law
ENDF/B-VII.1 U-234/235/238 happen to use. Closing the gap for other nuclides
needs the remaining ENDF secondary-energy laws with a real sampling algorithm,
plus the `NK > 1` case (multiple partial distributions mixed by their own
`p_k(E)` fraction) that any of them can appear inside.

- **njoy (`nuclear_data::secondary.rs`).** `FissionSpectrum` gains four variants:
  `Maxwell` (LF=7, θ(E)), `Evaporation` (LF=9, θ(E)), `WattEnergyDependent`
  (LF=11, a(E)/b(E)), and `Mixture` (NK>1, `Vec<(p_k TAB1, FissionSpectrum)>`,
  recursive via `Vec` — no `Box` needed). `from_endf_mf5` now loops over `NK`
  partitions, dispatching each on its ENDF **LF** code (the `L2` field of the
  partition's `p_k(E)` TAB1 header) via a `parse_mf5_section` helper factored out
  for unit testing. **LF=5** (general evaporation) and **LF=12** (Madland-Nix)
  abort the whole parse to `None` (Watt fallback) rather than partially
  succeeding: `Σₖ p_k(E) = 1` is a physical constraint, so a partition this port
  can't reconstruct makes the whole distribution wrong, not just incomplete.
  LF=5 specifically has **no sampling algorithm even in canonical OpenMC**
  (`GeneralEvaporation.to_hdf5` raises `NotImplementedError` in
  `openmc/data/energy_distribution.py`) — a genuine upstream gap, not ours.
- **outram-mc-libs (`material::nuclide.rs`).** `sample_fission_energy` now
  delegates to a free `sample_chi` dispatching all six `FissionSpectrum`
  variants. Three new samplers are direct ports of OpenMC
  `src/distribution_energy.cpp`: `sample_maxwell_lf7` (`MaxwellEnergy::sample`,
  itself calling `maxwell_spectrum` from `src/random_dist.cpp`, already ported as
  `rng::distributions::maxwell`), `sample_evaporation_lf9`
  (`Evaporation::sample`, the rejection inversion `x = −ln[(1−vξ₁)(1−vξ₂)]`), and
  `sample_watt_lf11` (`WattEnergy::sample`, reusing the already-ported `watt`
  sampler with energy-dependent a(E)/b(E)). `Mixture` samples which partition is
  active by its `p_k(e_in)` weight, then recurses.

### Results (2026-07-03, offline unit tests — no live Godiva nuclide uses these
laws, so validation is against closed-form distribution moments, not k_eff)

njoy parser (5 tests, hand-built ENDF rows, `nuclear_data::secondary::tests`):
correct variant + field extraction for LF=7, LF=9, LF=11, and an NK=2
(Maxwell+Evaporation) mixture; LF=5 confirmed falls back to `None`.

outram-mc-libs sampler (5 tests, `material::nuclide::tests`, N=200 000–2 000 000
draws per case): empirical mean matches the closed-form distribution mean within
the stated Monte Carlo tolerance, and every draw respects the restriction energy
`E' ≤ E − U`:

| Law | Setup | Theoretical mean | Sampled mean |
|---|---|---|---|
| LF=7 Maxwell | θ=1 MeV, u=0, e_in≫θ | 1.5θ = 1.5 MeV | matched, ±3.0e4 eV tol |
| LF=9 Evaporation | θ=1 MeV, u=0, e_in≫θ | 2θ = 2.0 MeV | matched, ±4.0e4 eV tol |
| LF=9, restriction tight | e_in−u = 0.5 MeV ≪ 2θ | — | every draw ≤ 0.5 MeV (rejection verified to bite) |
| LF=11 Watt(E) | a=1 MeV, b=2.249e-6 eV⁻¹, u=0 | 1.5a+0.25a²b ≈ 2.06 MeV | matched, ±5.0e4 eV tol |
| NK=2 Mixture | two Maxwells, θ=0.1/10 MeV, p=0.8/0.2 | p-weighted: 3.12 MeV | matched, ±8.0e4 eV tol |

One measurement pitfall worth recording: the first mixture-test attempt used
`e_in = 5×θ₂` for the θ=10 MeV partition, assuming "restriction far above the
spectrum scale" — but a Gamma(1.5,θ) tail is not negligible at only 5θ, so the
LF=7 restriction genuinely truncated it and biased the sampled mean low. That
was the sampler working correctly, not a bug; the fix was more headroom
(`e_in = 20×θ₂`), not a code change.

### Finding

MF=5 coverage is now LF=1/7/9/11 + NK>1 (every law with a real ENDF/OpenMC
sampling algorithm); only LF=5 (unsupported upstream) and LF=12 (Madland-Nix,
rare) remain. This doesn't move Godiva's k_eff — U-234/235/238 are all LF=1,
NK=1, unaffected by this change — but removes the silent Watt-fallback for any
future nuclide whose MF=5 uses LF=7/9/11 or a mixture (e.g. many non-U
actinides use LF=9 or LF=11). Exercised by 5 njoy unit tests
(`nuclear_data::secondary::tests::{parses_lf7_maxwell, parses_lf9_evaporation,
parses_lf11_watt_energy_dependent, parses_nk2_mixture,
lf5_general_evaporation_falls_back_to_none}`) and 5 outram-mc-libs unit tests
(`material::nuclide::tests::{maxwell_lf7_matches_theoretical_mean_and_respects_restriction,
evaporation_lf9_matches_theoretical_mean_and_respects_restriction,
evaporation_lf9_restriction_actually_bites,
watt_lf11_matches_theoretical_mean_and_respects_restriction,
mixture_dispatches_by_partition_weight}`).

## 2026-07 — GASPR: gas-production cross sections (njoy)

### Methodology

Ported the NJOY2016 `gaspr.f90` module (~1150 lines) that computes total
production of the five light "gas" nuclides — H1 (proton), H2 (deuteron), H3
(triton), He3, He4 (alpha), ENDF MT=203–207 — from a reconstructed evaluation.
This is a post-processing/informational cross section (depletion, material
swelling), not a transport quantity: no secondary angle/energy law is needed
because it is only ever read (e.g. by a depletion post-processor), never
sampled in a collision.

Rather than reproducing NJOY's residual-nucleus mass-difference bookkeeping
(`izr`/`izg` in `gaspr.f90`), the port exploits that this crate's own
`MtReaction` enum already names each reaction's emitted particles (e.g.
`Mt45NnProtonAlpha`, `Mt23Nn3Alpha`). Since ENDF MT numbers 11, 16, 17, 22–45,
and 102–117 are *mutually exclusive* reaction final states in the modern
ENDF/B-VI+ "lumped-channel" representation, gas production reduces to a flat
yield-weighted sum `σ_gas(E) = Σ_mt n_particle(mt)·σ_mt(E)` over the
reconstructed MF=3 sections — no double counting, no per-reaction
mass-difference derivation. A `gas_yield(mt)` lookup table encodes the
standard ENDF-102 particle content for every gas-producing MT.

**Scope cut, documented in the module doc, not silently dropped**: the legacy
MT=600–849 detailed-breakup fallback `gaspr.f90` uses when an evaluation omits
the lumped channels (pre-ENDF/B-VI representation) is *not* ported — rare in
the ENDF/B-VII/VIII libraries this workspace targets. A nuclide using only
that legacy shape would under-count (each unlisted MT contributes 0), not
silently produce a wrong nonzero answer.

### Results (2026-07-03, unit tests — no live nuclide in the Godiva model
produces significant gas, so this is validated structurally, not against a
k_eff benchmark)

6 tests in `gaspr::tests`, all passing, exercising the physical claims:

| Test | Claim checked |
|---|---|
| `np_alpha_reaction_yields_he4_only` | single-channel MT=107 `(n,α)` → He4 only, other species exactly 0 |
| `disjoint_channels_sum_additively` | MT=107 + MT=22 (both alpha-producing) sum correctly at each energy (0.4+0.1=0.5, 0.4+0.3=0.7 barn) |
| `multi_particle_yield_is_weighted` | MT=23 `(n,n'3α)`: yield 3 not 1 (0.2 b × 3 = 0.6 b) |
| `two_species_channel_credits_both` | MT=45 `(n,n'pα)` credits both H1 and He4 at their full 0.8 b, independently |
| `non_gas_reactions_are_ignored` | elastic/capture/(n,2n) sections excluded from both the union grid and every species' sum |
| `species_mt_numbers_are_203_to_207` | `GasSpecies::mt()` matches the ACE/PENDF convention |

Full njoy suite after the change: **105 lib tests** (up from 99), all green
(`crates/njoy-outram-park-fork/scripts/test.sh`, capped run).

### Finding

Gas production for the modern lumped-channel ENDF representation is now a
~250-line pure-Rust module instead of requiring a faithful translation of
NJOY's 1150-line residual-mass bookkeeping, because the particle-content
information NJOY *derives* per-MT was already sitting in this crate's
`MtReaction` naming from earlier porting work. Not wired into any transport
path (outram-mc-libs) — this is nuclear-data-processing output only, consumed by
future depletion/materials tooling, not the k-eigenvalue driver.

## 2026-07 — HEATR: kinematic-limit KERMA, phases H1–H4 (njoy)

### Methodology

`heatr.f90` (~6.3k lines) computes MT=301 heating (KERMA) via a **photon
energy-balance method**: it needs MF=12–15/MF=6 photon-production data,
momentum conservation for capture recoil, and (separately) a Lindhard-partition
damage-energy calculation — too large for a single-pass port like GASPR
(1.15k lines). It was broken into 7 phases (`docs/porting-plan.md` §HEATR
sub-phases), each its own commit; **H1–H4 are done tonight**, H5–H7 deferred.

All four phases share one physical idea — the **kinematic-limit KERMA**:
every escaping *neutron* carries its kinetic energy away from the local
region; everything else (nuclear recoil, charged particles, and — per NJOY's
own documented fallback, "deposits all photon energy locally when \[photon\]
files are not available" — photon energy) deposits locally. `heatr.f90` itself
computes this exact quantity as a **check** (`kchk`) against its full
photon-transport method; here (`src/heatr.rs`) it is the primary result.

- **H1 — elastic (MT=2).** Two-body kinematics averaged over isotropic CM
  scattering: `H(E) = σ(E)·E·2A/(A+1)²`. Independently reproduces the textbook
  fact that hydrogen (A=1) loses on average exactly half its energy per
  elastic collision.
- **H2 — local-deposition reactions (MT=102, 103–117).** No escaping neutron
  (pure capture, or capture + charged particle(s) that stay local):
  `H(E) = σ(E)·(E+Q)` — all of `E+Q` deposits, since nothing carries energy
  away.
- **H3 — single-escaping-neutron reactions (MT=4, 22, 23, 28, 29, 32–36, 44,
  45, discrete levels 51–90).** Derived the two-body-with-Q generalization of
  H1: `H(E) = σ(E)·[E·2A/(A+1)² + Q/(A+1)]` (reduces exactly to H1 at `Q=0` —
  verified as a test). Caught a real bug mid-implementation: MT=51–90 have
  individually *named* enum variants (`Mt51NnLevel1`, …), not a generic
  `Unknown(n)` catch-all as assumed — the initial `Unknown(n) if
  (51..=90).contains(&n)` guard was dead code. Fixed by matching on
  `mt.number()` instead of variant shape.
- **H4 — fission (MT=18, 19–21, 38).** `H(E) = σ_f(E)·[E + Q_fission −
  ν̄(E)·⟨E'⟩]` — the multi-neutron generalization of H2/H3's energy balance:
  `ν̄` escaping neutrons each carry the birth spectrum's mean energy `⟨E'⟩`,
  everything else deposits locally. Reused this session's `NuBar` (ν̄) and
  added `FissionSpectrum::mean_energy` (⟨E'⟩ — closed-form for the analytic
  MF=5 laws, trapezoidal quadrature for the tabulated ones), a direct
  beneficiary of the earlier MF=5 porting work in the same session.

### Results (2026-07-04, unit tests — no live nuclide's real evaluation was
run through this path yet; validated against closed-form kinematics and
physical sanity bounds)

14 tests in `heatr::tests`, all passing:

| Test | Claim checked |
|---|---|
| `hydrogen_elastic_loses_half_its_energy` | H1 exact: A=1 ⇒ H=σE/2 |
| `heavy_target_transfers_small_fraction` | H1 asymptotic: H/(σE)→2/A as A→∞ |
| `capture_deposits_e_plus_q`, `charged_particle_only_exit_deposits_e_plus_q` | H2 exact: H=σ(E+Q) |
| `single_neutron_at_q_zero_matches_elastic_formula` | H3→H1 at Q=0 |
| `discrete_level_heating_is_reduced_by_negative_q` | H3: negative Q reduces H below the Q=0 term |
| `nn_alpha_family_uses_single_neutron_formula` | H3 applies to (n,n'α)-family MTs, not just discrete levels |
| `fission_heating_matches_energy_balance_formula` | H4 exact vs the closed-form Watt mean |
| `fission_heating_is_positive_and_order_200_mev` | H4 sanity: ~190 MeV/fission, not a sign error |
| `all_three_phases_sum_additively`, `all_four_phases_sum_additively`, `elastic_and_capture_sum_additively` | union-grid summation across reaction types |
| `not_yet_modeled_reactions_contribute_nothing` | H5 (multi-neutron exit) correctly excluded, not silently wrong |
| `eval_interpolates_between_grid_points` | lin-lin interpolation on the union grid |

Full njoy suite after H4: **121 lib tests** (up from 105 pre-HEATR), all green
(`crates/njoy-outram-park-fork/scripts/test.sh`, capped run).

### Finding

The kinematic-limit KERMA is a real, if approximate, NJOY code path (not an
invented shortcut) — it's what `heatr.f90` itself falls back to for
materials/reactions without photon-production data. Four phases now give
MT=301 heating for the reactions that dominate a typical fast-spectrum
material's energy deposition (elastic, capture, discrete inelastic, fission);
the remaining gap (H5: multi-neutron-exit + continuum inelastic, contributing
0 for now) is a minority of the total cross section for most nuclides. H6
(the full photon energy-balance method) and H7 (damage energy) remain
deferred — genuinely larger undertakings needing MF=12–15/MF=6 photon data
and a separate Lindhard-partition model, not reachable in this session.
