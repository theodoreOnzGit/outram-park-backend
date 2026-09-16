# Transport-kernel physics: what is implemented, verified, and ablatable

**Date: 2026-09-16.** A systematic enumeration of every physics mechanism in
`outram-mc-libs`' collision kernel and every module of `njoy-outram-park-fork`,
against three independent questions:

| column | question |
|---|---|
| **impl** | is the mechanism implemented at all? |
| **verified** | is there a test comparing it to something outside itself? |
| **ablatable** | can it be switched off *in-process*, so its worth can be priced? |
| **control** | is there a test asserting the ablation actually ablates? |

**The fourth column is the one that is usually missing, and it is not a
formality.** An ablation hook with no control test reports "no difference" when
it silently fails, and that reads as "this physics does not matter" — the worst
failure mode an ablation study has, and one this crate has hit before
(`op-50vu`: free-gas and bound eigenvalues came out bit-identical because the
S(α,β) wiring was not reaching the sampler).

**Ablatable means in-process.** A mechanism switchable only through an
environment variable read once per process cannot be used for a paired-seed
study, because both arms cannot exist at once over the same seeds. Those are
marked `env` and count as a gap, not as coverage.

---

## `outram-mc-libs` — the collision kernel

| mechanism | impl | verified against | ablatable | control |
|---|---|---|---|---|
| Elastic energy (two-body, target at rest) | ✅ | analytic two-body closed form | n/a (not removable) | n/a |
| **Elastic angular** (MF=4/MT=2) | ✅ | quadrature vs CDF inversion, 8 nuclides × 5 energies | ✅ `with_isotropic_elastic_scattering` | ✅ ×3 |
| **Free-gas target motion** | ✅ | analytic Doppler integral (≤0.09 % on H-1); Maxwellian fixed point | ✅ `with_target_at_rest` (closed 2026-09-16) | ✅ ×2 |
| **S(α,β) bound thermal** | ✅ | NJOY THERMR; detailed-balance fixed point | ⚠️ **`env` only** (`OUTRAM_RINGRPT_FREE_GAS_GRAPHITE`) | ✅ (6 files touch `with_thermal_scattering`) |
| Discrete inelastic energy (MT=51…90) | ✅ | ENDF level table; threshold invariant | ✅ `without_inelastic` | ✅ (closed 2026-09-16) |
| **Discrete inelastic angular** (MF=4/MT=51…90) | ✅ | per-level `⟨μ_cm⟩` vs OpenMC ACE (3.1e-3) | ✅ `with_isotropic_inelastic_scattering` | ✅ ×2 |
| **Continuum inelastic energy** (MF=6 LAW=1 `f₀`) | ✅ | NJOY/ENDF tape; `⟨E'/E⟩` | ✅ `without_evaluated_continuum` | ✅ ×1 |
| **Continuum inelastic angular** (MF=6 `LANG=1`) | ✅ | tape's own `a₁` (2.2e-4) | ✅ `with_isotropic_continuum_scattering` | ✅ (closed 2026-09-16) |
| **Continuum inelastic angular** (MF=6 `LANG=2` Kalbach) | ✅ | closed form `r·(coth a − 1/a)` | ✅ (same hook) | ✅ (same) |
| (n,2n) MT=16 multiplicity | ✅ | ENDF yield, per-subsection sum | ✅ `with_unit_n2n_multiplicity` (closed 2026-09-16) | ✅ ×2 (incl. a **kernel-level** one) |
| **(n,3n) MT=17 multiplicity** | ✅ (2026-09-16) | ENDF MT=17 σ + MF=6 MT=17 law; threshold invariant | ✅ same hook (scope widened) | ✅ ×2 |
| Tabulated **source** energy distribution | ✅ (2026-09-16) | uniform-CDF inversion; degenerate-table bounds | n/a | ✅ ×2 |
| **DBRC / resonance elastic upscatter** | ✅ (2026-09-16) | 0 K elastic structure (175× across U-238's 6.67 eV resonance); inert above its limit; `None` path bit-identical | ✅ `without_dbrc` | ✅ ×3 |
| ν̄(E) energy dependence | ✅ | MF=1/452 tape | ✅ `with_frozen_nubar` (closed 2026-09-16) | ✅ ×2 |
| χ(E→E') fission spectrum (MF=5) | ✅ | MF=5 tape; `⟨E_out⟩` vs OpenMC (0.018 %); **sampler vs the tape's own row means, ≤1 %** | ✅ `with_frozen_fission_spectrum` (closed 2026-09-16) | ✅ ×2 |
| Threshold behaviour (σ = 0 below threshold) | ✅ | ENDF redundancy relation; gh:#193 gate | n/a | n/a |
| Delta (Woodcock) tracking | ✅ | vs surface-tracked CSG (18 pcm) | ✅ (method choice) | ✅ |
| CSG geometry / surface tracking | ✅ | analytic intersections; lattice overlap | n/a | n/a |
| RNG stream independence | ✅ | seed-to-seed `sd` gate (`op-rbo`) | n/a | ✅ |
| **URR probability tables** | ✅ (2026-09-16) | **NJOY2016 PURR: Bondarenko elastic 4.2e-7, capture 3.0e-7** | ✅ `without_urr_probability_tables` | ✅ ×3 |
| MF=6 incident-energy interpolation flag | 🟡 **carried, not honoured** (2026-09-16) | measured: 17 ranges INT=22 unit-base, **9 ranges INT=12 corresponding-point** | n/a | ✅ survey gate |
| MF=6 `LANG = 11…15` (tabulated cosines) | ❌ retained, unsampled | — | n/a | n/a |

## `njoy-outram-park-fork` — the data path

| module | port status | verified against |
|---|---|---|
| RECONR (SLBW/MLBW/Reich-Moore) | ✅ | NJOY2016 PENDF, 7 s.f. |
| RECONR LRF=4 (Adler-Adler) | ✅ | ⚠️ **no LRF=4 evaluation held** (gh:#172) |
| RECONR LRF=7 (R-matrix limited) | ✅ | NJOY2016, 44 326 pts, worst 9.8e-3 |
| BROADR | ✅ | NJOY2016; `thnmax` bound (`op-sdbk`) |
| UNRESR / PURR | ✅ kernel + transport-facing tables; PENDF MT=152/153 *writer* still unported | ✅ NJOY2016, converged Bondarenko moments to 4.2e-7 (elastic) / 3.0e-7 (capture) |
| THERMR | ✅ | NJOY2016 MF=6, 7 s.f. after gh:#188 |
| ACER | ✅ | NJOY2016 ACE, 5.3e-11 |
| GROUPR / GAMINR / COVR / ERRORR / LEAPR | ✅ | NJOY2016 |
| WMP | 🟡 `from_blob` TODO |
| MF=5 LF=5 (general evaporation) | ❌ not ported | no sampling consumer |

---

## The gap list, in priority order

Priority is by **flux-weighted worth × absence of a control**, not by how easy
each is.

### ~~1. `with_isotropic_continuum_scattering` has no control test~~ — CLOSED 2026-09-16

The hook `examples/godiva_continuum_anisotropy_ablation.rs` depends on. Its
existing control (`tests/continuum_angular_ablation_control.rs`) tests
`ContinuumAngularMode` — the *enum passed to the scatter function* — not the
`Nuclide`-level hook the example actually calls. Those are different code
paths, and only the untested one is used to produce a number.

### ~~2. `without_inelastic` has no control test~~ — CLOSED 2026-09-16

It removes the largest single reactivity mechanism this crate has measured
(−4190 pcm on the FHR pebble, the row that localised gh:#193 to F-19). A silent
failure here would have derailed that investigation.

### ~~3. Free-gas target motion is not ablatable in-process~~ — CLOSED 2026-09-16

Worth **−2242 pcm** on the FHR pebble — the positive control in gh:#193's
pricing table and the single biggest effect in it. It was reachable only by
zeroing the transport temperature through one example's environment variable,
so it could not take part in a paired-seed study alongside the other
mechanisms.

**Closed by `Nuclide::with_target_at_rest`**, a per-nuclide flag consulted
through `Nuclide::free_gas_kt(temp_k)`. Every transport driver now takes its
elastic-kinematics temperature from that one call rather than multiplying
`K_BOLTZMANN_EV_PER_K` itself (5 sites in `keff.rs`/`transport_csg.rs`/
`keff_delta.rs`, 2 in `slowing_down.rs`), so the hook cannot be reachable from
one driver and not another — the constant is no longer imported by any of them.
The ablation is expressed *through* the production path: a zero `kT` makes
`free_gas_elastic_scatter` take its own target-at-rest branch, so no branch was
added to the transport kernel.

Controls in `tests/ablation_hook_controls.rs` (2 tests, both passing):

- **It ablates, and had something to ablate.** Measured on ENDF/B-VIII.0 U-238
  at 293.6 K: `kT 2.530049e-2 -> 0` eV, and up-scatter — the signature of target
  motion, since a target at rest can only take energy away — goes
  **4207/8192 -> 0/8192** at 0.0253 eV. The ablated arm's outcomes are confined
  to `[2.487485e-2, 2.529998e-2]` eV, inside `[α·E, E]` with `α = 0.98319`, and
  cross sections are bit-identical across the hook.
- **It is a bit-for-bit no-op above `400·kT`.** 2048/2048 paired draws at 2 MeV
  give identical outgoing energy *and* leave the RNG streams in lockstep
  (threshold `1.0120e1` eV). This is
  what makes the recorded prediction — ~zero worth on a bare fast metal sphere,
  whose flux is almost all above the 10.12 eV threshold — a prediction rather
  than a hope: a non-zero Godiva reading would mean the wiring, not the physics.

**One thing this hook does NOT have, stated because the others do.** It breaks
RNG-stream invariance. The free-gas kernel draws a target velocity (a rejection
loop plus a rotation) that the target-at-rest kernel never draws, so the two
arms diverge at the first thermal collision. The angular ablations swap one
sampled quantity for another drawn from the same number of variates and stay in
lockstep history by history; this one is attributable **statistically over an
ensemble of seeds only**. A single paired run measures nothing here, and the
doc comment says so.

### ~~4. ν̄(E) and χ(E→E') cannot be ablated at all~~ — CLOSED 2026-09-16

Both are verified against the tape and against OpenMC, so their *data* was
sound. Neither could be priced. For a bare fast sphere ν̄(E)'s slope is a direct
reactivity lever, and it was the one mechanism in the fast kernel with no way to
ask "what is it worth".

**Closed by `Nuclide::with_frozen_nubar(e_ref)` and
`Nuclide::with_frozen_fission_spectrum(e_ref)`**, routed through `nu_bar(e)` and
a private `chi_incident_energy(e_in)` the way the free-gas hook is routed
through `free_gas_kt`. Both *freeze* rather than remove: a zero ν̄ is not an
ablation, it is a subcritical block of metal, and the arms would differ by the
whole eigenvalue instead of one mechanism's worth. What is worth pricing is the
**slope**.

Controls in `tests/ablation_hook_controls.rs` (3 tests), measured on
ENDF/B-VIII.0 U-235 at 293.6 K:

- **ν̄** — `2.42985` at 0.0253 eV against `2.64574` at 2 MeV, a rise of
  `0.21589`. Frozen at thermal it reads `2.42985` at every energy and
  `nu_fission` at 2 MeV falls `3.40904 -> 3.13086 b`, **−8.16 %**, with cross
  sections bit-identical.
- **χ** — LF=1 on **22 incident rows**, 1.000e-5 … 3.000e7 eV. The tape's own
  pdfs integrate to mean birth energy `1.99980e6` eV at 1e-5, `2.01746e6` eV at
  14 MeV (**+0.883 %**), `2.30184e6` eV at 30 MeV (**+15.103 %**), and the
  **sampler reproduces both endpoint rows to within 1 %**.
- **They compose** with each other and with the scattering hooks, so one
  paired-seed study can vary several mechanisms without confounding them.

**A prediction this licenses, recorded before measuring it.** The two factors of
the fission source are very unequal levers on a fast system: freezing ν̄ costs
8.16 % of `nu_fission` at 2 MeV — thousands of pcm — while χ's dependence is
worth 0.883 % in mean birth energy across the entire range a fission spectrum
occupies, so freezing χ should move `k` by **well under 100 pcm on Godiva**. A
larger χ reading means the wiring, not the physics.

**A methodological correction worth keeping.** The χ control first asserted "the
sampled mean must move more than 1 % from thermal to 14 MeV" and **failed at
+0.904 %**. The arbitrary threshold was the defect, not the sampler — reading
the tape showed χ barely moves through the fission-spectrum range and hardens
only above ~15 MeV, where third-chance fission sets in. The test now takes its
"there was something to remove" condition from the evaluation's own rows and
additionally cross-checks the sampler against them, which is a stronger claim
than any threshold on the difference would have been. Same lesson as the
`without_inelastic` partition assumption two commits earlier: state what the
data says, then assert it — do not assert what it ought to say.

**Known partial no-op, stated because it is silent.** On the LOW (`Core`) tier
*above* the WMP `e_max`, `nu_fission` comes from fast MGXS group data with ν̄
already baked into the group constant, so there is no separate ν̄ factor for
`with_frozen_nubar` to freeze up there. It is complete on the HIGH
(`Pointwise`) tier, which every case that would ask this question runs on.

### 5. `op-os8x` — the open physics question, now localised

Measured on HEAD 2026-09-16 against OpenMC on identical data: mean `E`
**+0.42 %** (4.6 σ), flux below 300 keV **−1.22 %** (6.0 σ), while `k` agrees to
`−32 ± 34 pcm`. Localised per-bin to **+0.88 % excess flux at 1.9–3.0 MeV**
(4.7 σ, 13.8 % of the flux) against **1.4–1.9 % deficits at 67–174 keV**.

Only inelastic scattering moves a 2 MeV neutron to ~100 keV in one collision.
Excluded by measurement: the angular laws (both in, and the continuum one's
ablation shows it does not move the spectrum), the cross sections (≤0.06 %
flux-weighted), and `k`.

### The leading suspect is now EXCLUDED too (2026-09-16)

**The MT=91 continuum `f₀(E→E')` shape was the leading suspect. It is not the
cause.** `tests/mt91_transfer_vs_openmc.rs` compares our law against OpenMC's
own, extracted from the HDF5 built in-session off the same ENDF/B-VIII.0 tape,
across all **18** incident rows in 1.5–3.5 MeV. Mean outgoing energy, median and
`P(E' < 300 keV)` agree on **every row to the 7 significant figures printed** —
worst deviation `0.0000 %`, signed bias `−0.0000 %`. The two are the same table.

**A correction to the method recorded on this gap.** The measurement previously
named here was "a per-MT collision tally in the 1.9–3.0 MeV band on both sides".
Working it through, **that measurement cannot discriminate**: a collision rate is
flux × σ, the cross sections already agree to ≤0.06 % flux-weighted, so a rate
comparison would largely restate the flux difference it is meant to explain.
What discriminates is the **transfer** — where an MT=91 collision at 2–3 MeV puts
the neutron — and that needs no transport, no seeds and no statistics at all.

### What that leaves

Cross sections, angular laws, `k`, the transfer table, the within-row CDF
inversion and the inter-row unit-base rule are **all excluded by measurement**.
`op-os8x` remains unexplained. What is left:

1. **The CM→lab transform on the continuum law.** Both codes store this law in
   the centre-of-mass frame (`LCT=2`; OpenMC reports `center_of_mass == True`)
   and each transforms at sampling time. The transform couples the sampled
   `μ_cm` to `E'`, so a difference here moves the spectrum while leaving every
   table identical — exactly the signature left.
2. ~~**Inter-row (unit-base) interpolation.**~~ **MEASURED AND EXCLUDED
   2026-09-16.** Our sampled `⟨E'⟩` at 8 incident energies deliberately
   *between* tabulated rows agrees with the closed-form mean of OpenMC's
   unit-base construction to a worst `0.0228 %` and a flat signed `−0.0205 %`,
   with no energy dependence. The within-row CDF inversion goes with it: on-grid
   `r = 0` makes the rescale the identity, and the sampled mean sits the same
   flat `−0.05 %` from the exact row mean there.

   **This one nearly became a false positive, and the near-miss is the point.**
   The first run reported a systematic `+0.60 %` bias growing to `+1.76 %` with
   energy — precisely the `op-os8x` signature — and it was written up as a
   probable cause. The defect was in the **reference**: the mean was computed as
   `∫E·p dE / ∫p dE` by the trapezoid rule on both sides. The denominator is
   exact (`p` is linear between points); the numerator is not (`E·p(E)` is
   quadratic), and its error `−h³m/6` grows with bin width and pdf slope, hence
   with incident energy. Against the exact lin-lin integral the sampler is a
   flat `−0.05 %` at 1.945, 2.4 and 3.0 MeV, where the trapezoid error ran
   `+0.089 %`, `−0.259 %`, `−1.632 %`.

   **Fifth reference-side error in this study.** The four "nearest-point trap"
   instances in the crate's `CLAUDE.md` were about reading a table at the wrong
   index; this one is about integrating it with the wrong rule. The general
   form is the same: *a reference is not right merely for being external, or for
   being the obvious formula.*

   It also qualifies the row-by-row result above. That comparison applied the
   *same* inexact rule to the *same* tables, so its exact agreement was partly
   guaranteed. It still establishes what it claims — the tables are identical —
   but it was never evidence that either side's moment was right. Both the test
   and the oracle scripts now integrate the first moment in closed form.
3. **The competing-channel branching at 2–3 MeV** — which MT a collision is
   assigned to, as distinct from the cross sections themselves.

Of the three, only the CM→lab transform and the channel branching are still
open — item 2 was measured and excluded (above). **Neither of the remaining two
has been measured.** They are where to look next, not findings.

### 6. Two public names for one ablation (the (n,2n) half is now closed)

Found while closing gaps 3 and 4, and recorded rather than silently changed —
renaming a public method on a crate the maintainer has declared mature is their
decision, not an agent's.

- **`Nuclide::with_isotropic_elastic_scattering` and
  `Nuclide::with_isotropic_elastic` are the same hook.** Both clear
  `elastic_angular` to its default and nothing else; the bodies are identical
  up to `ElasticAngular::default()` versus `Default::default()`. Both are in
  use — the first from two call sites, the second from
  `tests/elastic_anisotropy_vs_endf_mf4.rs`. Two names for one behaviour is
  precisely the discoverability failure the root `CLAUDE.md` "Human interface
  layer" section is about: a reader hovering one has no way to learn the other
  exists, and a study that cites "the elastic ablation" is ambiguous about
  which it ran even though the answer is the same. Proposed fix: keep
  `with_isotropic_elastic_scattering` (the longer name matches its three
  siblings, `with_isotropic_inelastic_scattering` and
  `with_isotropic_continuum_scattering`), migrate the one test, and delete the
  short one.
- ~~**(n,2n) MT=16 multiplicity still has no dedicated control.**~~ **CLOSED
  2026-09-16** by `Nuclide::with_unit_n2n_multiplicity`, which cuts the yield
  from 2 to 1. The emission *law* on MT=16 was already covered
  (`without_evaluated_continuum`, `with_isotropic_continuum_scattering`); the
  **yield** was not, and was ablatable only lumped in with everything else via
  `without_inelastic`.

  **The secondary is drawn either way and only its *emission* is gated.**
  Skipping the draw would have saved a few variates and desynchronised the two
  arms; drawing it keeps them in exact lockstep, so the paired difference is
  deterministic on a shared seed. That is a strictly better-conditioned
  ablation than `with_target_at_rest`, which cannot have this property.

  Measured: U-238 `σ_n2n = 1.45833 b` at 12 MeV and exactly 0 below threshold
  at 2 MeV; cross sections (σ_n2n included) bit-identical across the hook and
  the MT=16 energy law intact. At the kernel, a bare U-235 sphere on one shared
  seed gives `k` **0.973390 → 0.972432, −95.8 pcm** — negative, as it must be,
  since deleting a neutron source cannot raise the eigenvalue. That is **one
  paired sample on one seed and one material, not the Godiva worth**; pricing it
  is ensemble work for a bigger machine.

  **This closure found a real defect, and it is the reason the kernel-level test
  exists.** On its first run the two arms came out bit-identical: the hook had
  been wired into four of the **five** emission sites, and the one missed was in
  `physics::keff`'s `transport_history` — the path `run_keff` actually takes. A
  data-level control would have passed and the hook would have shipped reporting
  that (n,2n) multiplicity is worth nothing, which is precisely `op-50vu`
  recurring. Every new ablation hook whose mechanism lives in the transport
  kernel should get a kernel-level control, not only a data-level one.

### 7. Known-absent physics, correctly documented

~~URR probability tables~~ **— CLOSED 2026-09-16.** PURR is now verified
against NJOY2016 and wired into transport; see the row above and
`crates/njoy-outram-park-fork/tests/purr_u238_ptables_vs_njoy.rs`. What
remains unported is the **PENDF MT=152/153 tape writer**, which transport does
not need — `outram-mc-libs` never reads a PENDF, it reads ENDF and builds a
`Nuclide` — so the writer matters only for NJOY interoperability.

**Three more found by re-surveying on 2026-09-16, two already fixed:**

- ~~**(n,3n) MT=17 had no branch in any kernel.**~~ **FIXED.** MT=17 is inside
  MT=1, so the collision happened but fell through to the *elastic* arm and both
  extra neutrons were silently lost. Now evaluated, branched on in all five
  kernels, and emitting two extras from the MF=6 MT=17 law. Measured: U-238
  `σ_n3n = 0.43176 b` at 14 MeV and **exactly 0** at every reactor-spectrum
  energy — so the new arm's bound coincides with the old `else` boundary there
  and the partition is bit-identical, which the control asserts directly.
- ~~**`TabulatedEnergy::sample` was `todo!()`.**~~ **FIXED.** A constructible
  source distribution that aborted the run if anything drew from it. Nothing in
  the crate constructed one, which is exactly how a latent panic survives a test
  suite. Now a piecewise-linear CDF inversion with two tests, including
  degenerate tables.
- ~~**DBRC (Doppler Broadening Rejection Correction) is absent.**~~
  **IMPLEMENTED 2026-09-16.** Free-gas elastic used the constant-cross-section
  approximation and never resampled σ at the relative energy inside a resonance.
  Now a second rejection on `σ_s^{0K}(E_rel)/σ_max`, layered on upstream's own
  relative-speed rejection as two independent accept tests (which is what makes
  the product of the weights right, and is what OpenMC does).

  The 0 K elastic grid is retained at construction — up to `DBRC_GRID_MAX_EV`
  = 25 keV, clearing U-238's 20 keV resolved range — because the target motion
  is modelled explicitly and a *broadened* σ would count Doppler broadening
  twice. Measured: 34 916 points below 1 keV, and the 6.67 eV resonance towers
  **175×** over the potential scattering at 3 eV. Opt-in via `with_dbrc(e_max)`;
  1 keV is OpenMC's default.

  **Not priced.** The literature puts DBRC at order 100–200 pcm in an LWR pin
  cell. This crate cannot see that on Godiva — a bare fast sphere has
  essentially no flux in U-238's resolved resonances — so measuring it needs a
  thermal or epithermal case. That is the same gap recorded below as the
  project's largest, and DBRC is now a second reason to close it.

**The MF=6 incident-energy interpolation flag — measured 2026-09-16, and the
finding is not what the survey assumed.** It was recorded as "dropped", which
was true: the parser read the TAB2's `INT` and `ChiTabular` threw it away, so
no consumer could see what the evaluation asked for. It is now **carried**.

What it says is more interesting than expected. ENDF File 6 uses the extended
codes — `11..=15` is *corresponding-point* interpolation, `21..=25` is
*unit-base* — not the plain `1`/`2` a reader would assume. Across this
workspace's 27 neutron tapes on MT=16/17/91: **17 ranges are INT=22 (unit-base)
and 9 are INT=12 (corresponding-point)**. Our samplers apply unit-base to all
of them, so roughly a third get the wrong rule *relative to the evaluation*.

**It is not a discrepancy against our reference implementation**, which is what
makes this a maintainer decision rather than a fix. NJOY's ACER preserves the
flag into the ACE Law-4 header, OpenMC applies unit-base regardless, and this
port's sampled spectra reproduce OpenMC's construction to 0.02 %
(`mt91_transfer_vs_openmc.rs`). Honouring corresponding-point would make this
crate deviate from the code it is a port of. Carried, measured, documented —
deliberately not acted on unilaterally.

The survey that produced those counts is a permanent in-crate gate
(`acer::energy::mf6`'s `survey_incident_energy_interpolation_laws`) and counts
its own skips, so it cannot report a clean answer about a fraction of the data.
It found two: H-2 and Be-9 MT=16 use **LAW=6/LAW=7**, not LAW=1 — a separate,
previously unrecorded gap in the continuum path.

**MF=6 LAW=6 (phase space) — CLOSED 2026-09-16.** H-2's MT=16 was falling back
to the Weisskopf stand-in with nothing recording it. `from_endf_mf6` now
converts LAW=6 into the tabulated form the samplers already consume: the law's
shape is incident-independent in `x = E'/E'_max`, so evaluating
`E'_max(E) = ((APSX−AWP)/APSX)·(AWR/(AWR+1)·E + Q)` (upstream `f6psp`,
`groupr.f90:12658`) on an incident grid reuses the entire existing sampler —
**no new sampling path, no kernel change, no second implementation to drift**.
Angular is `EvaluatedIsotropic`, which phase space is by construction.
Verified against upstream's closed form: **worst relative shape difference
0.0000** over 7 incident rows, `E'_max` 7.3042e4 → 6.5133e7 eV.

**MF=6 LAW=7 (Be-9 MT=16) is still unported**, and a test now *asserts* that it
returns no law — so the day it is implemented the test fails and gets updated,
rather than the gap persisting behind a fallback nobody rechecks.

**MF=6 `LANG = 11…15` and MF=5 LF=5 — measured 2026-09-16, and both turn out to
be unused by any held evaluation.** They are now *enforced* absences rather than
documented ones: two survey gates assert it, so a tape using either fails loudly
instead of silently degrading.

- **`LANG = 11…15`** (tabulated cosines): across 27 tapes only `Legendre` and
  `KalbachMann` appear, both implemented. A tape using 11…15 would have fallen
  back to isotropic without a word.
- **MF=5 `LF`**: only **LF=1** (12 subsections) and **LF=9** (1) appear, both
  ported. An unported LF makes the *whole* MF=5 fall back to the thermal-Watt
  stand-in, so the gate matters more than the count suggests.

**A wrong comment corrected while doing it.** The code said LF=5 was
"unsupported upstream". **NJOY supports it** — `groupr.f90:12355` is literally
"law 5. general evaporation spectrum", with `acefc.f90` handling it in three
more places. That is the kind of error that stops someone porting something,
and the comment now records both the correction and the fact that LF=5 is a
tabulated `g(x)` with `x = E'/θ(E)` — the same "universal shape, moving scale"
form as LAW=6, so it would convert the same way if a tape ever needs it.

Still open: MF=6 LAW=7 (Be-9 MT=16). These are gaps but not defects — each is recorded where a reader meets
it.

---

## The thermal validation gap — CLOSED 2026-09-16

This document's standing caution was that the project targets **thermal**
reactors while nearly all its validation was a bare fast sphere, and that
LCT-008 — the one thermal benchmark — had not been re-run since GitHub #188's
thermal-kernel fix. It has now been run, and #188's own stated criterion ("the
three `Δk` must collapse together and toward zero") is met on both counts:

| case | 2026-09-13 | **2026-09-16** |
|---|---|---|
| 1 | +2665 ± 128 pcm | **+157 ± 119 pcm** |
| 2 | +2086 ± 118 pcm | **+1 ± 126 pcm** |
| 8 | +1605 ± 114 pcm | **+124 ± 124 pcm** |

Spread across the three: **1060 → 156 pcm**. Mean `|Δk|`: **94 pcm**, every case
within 1.3 σ of a measured critical experiment. The pairwise differences — which
are truly zero, since every case is independently critical — went from
**+579 ± 174 (3.3 σ)** and **+481 ± 164 (2.9 σ)** to **−156 ± 173 (0.9 σ)** and
**+123 ± 177 (0.7 σ)**. *"Its boron is worth too little"*, this case's
load-bearing finding for weeks, is resolved.

**DBRC and URR were then priced on this case, and both are BOUNDED rather than
resolved** (`--dbrc` / `--urr`, added to `lct008_keff`; both default off so the
no-flag arm reproduces the baseline):

| arm | `Δk` | worth |
|---|---|---|
| neither | +157 ± 119 pcm | — |
| `--dbrc` | +197 ± 129 | **+40 ± 176 (0.2 σ)** |
| `--urr` | +60 ± 132 | **−97 ± 178 (0.5 σ)** |
| both | +210 ± 129 | **+53 ± 176 (0.3 σ)** |

All consistent with zero. **None is a measurement**: `σ_diff ≈ 176 pcm` here, so
a single paired run resolves only effects above **~350 pcm at 2 σ**, and the
published DBRC value for an LWR pin cell (100–200 pcm) sits *below* that. This
does not contradict the literature — it cannot see an effect that size.
Resolving either needs a paired-seed ensemble, which is CPU rather than physics.

The run did establish one thing beyond the bound: **the URR wiring generalises**
past U-238, building tables for all three actinides (U-234, U-235, U-238) in
~5 s total. The control test could only show U-238.

## What "correctly represented" would mean

Every row of the first table having ✅ in all four columns, or an explicit
recorded reason why a column does not apply. That is not the state today and
this document is the list of what stands between.

The column that matters is **control**. A mechanism can be implemented and
verified and still be un-priceable, and an un-priceable mechanism is one whose
contribution to a residual like `op-os8x` cannot be tested — which is precisely
the position this crate was in before the ablation controls existed.
