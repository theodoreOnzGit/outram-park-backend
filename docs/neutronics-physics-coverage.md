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
| (n,2n) MT=16 multiplicity | ✅ | ENDF yield, per-subsection sum | ⚠️ via `without_inelastic` (lumped) | ❌ none |
| ν̄(E) energy dependence | ✅ | MF=1/452 tape | ❌ **no hook** | ❌ none |
| χ(E→E') fission spectrum (MF=5) | ✅ | MF=5 tape; `⟨E_out⟩` vs OpenMC (0.018 %) | ❌ **no hook** | ❌ none |
| Threshold behaviour (σ = 0 below threshold) | ✅ | ENDF redundancy relation; gh:#193 gate | n/a | n/a |
| Delta (Woodcock) tracking | ✅ | vs surface-tracked CSG (18 pcm) | ✅ (method choice) | ✅ |
| CSG geometry / surface tracking | ✅ | analytic intersections; lattice overlap | n/a | n/a |
| RNG stream independence | ✅ | seed-to-seed `sd` gate (`op-rbo`) | n/a | ✅ |
| **URR probability tables** | ❌ **absent** | — (ablated on the *OpenMC* side: +43 ± 38 pcm) | n/a | n/a |
| `EnergyAngular` interpolation flag | ❌ **dropped** | — | ❌ | ❌ |
| MF=6 `LANG = 11…15` (tabulated cosines) | ❌ retained, unsampled | — | n/a | n/a |

## `njoy-outram-park-fork` — the data path

| module | port status | verified against |
|---|---|---|
| RECONR (SLBW/MLBW/Reich-Moore) | ✅ | NJOY2016 PENDF, 7 s.f. |
| RECONR LRF=4 (Adler-Adler) | ✅ | ⚠️ **no LRF=4 evaluation held** (gh:#172) |
| RECONR LRF=7 (R-matrix limited) | ✅ | NJOY2016, 44 326 pts, worst 9.8e-3 |
| BROADR | ✅ | NJOY2016; `thnmax` bound (`op-sdbk`) |
| UNRESR / PURR | 🟡 kernel ported, MT=152/153 output partial | ⚠️ PURR **never run** |
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

### 4. ν̄(E) and χ(E→E') cannot be ablated at all

Both are verified against the tape and against OpenMC, so their *data* is
sound. Neither can be priced. For a bare fast sphere ν̄(E)'s slope is a direct
reactivity lever, and it is the one mechanism in the fast kernel with no way to
ask "what is it worth".

### 5. `op-os8x` — the open physics question, now localised

Measured on HEAD 2026-09-16 against OpenMC on identical data: mean `E`
**+0.42 %** (4.6 σ), flux below 300 keV **−1.22 %** (6.0 σ), while `k` agrees to
`−32 ± 34 pcm`. Localised per-bin to **+0.88 % excess flux at 1.9–3.0 MeV**
(4.7 σ, 13.8 % of the flux) against **1.4–1.9 % deficits at 67–174 keV**.

Only inelastic scattering moves a 2 MeV neutron to ~100 keV in one collision.
Excluded by measurement: the angular laws (both in, and the continuum one's
ablation shows it does not move the spectrum), the cross sections (≤0.06 %
flux-weighted), and `k`. **Leading suspect: the MT=91 continuum `f₀(E→E')`
shape.** The discriminating measurement is a per-MT collision tally in the
1.9–3.0 MeV band on both sides.

### 6. Known-absent physics, correctly documented

URR probability tables (absent; priced on the OpenMC side at +43 ± 38 pcm,
consistent with zero), the `EnergyAngular` interpolation flag, MF=6
`LANG = 11…15`, MF=5 LF=5. These are gaps but not defects — each is recorded
where a reader meets it.

---

## What "correctly represented" would mean

Every row of the first table having ✅ in all four columns, or an explicit
recorded reason why a column does not apply. That is not the state today and
this document is the list of what stands between.

The column that matters is **control**. A mechanism can be implemented and
verified and still be un-priceable, and an un-priceable mechanism is one whose
contribution to a residual like `op-os8x` cannot be tested — which is precisely
the position this crate was in before the ablation controls existed.
