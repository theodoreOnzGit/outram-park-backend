# Development history — neutronics k-eigenvalue path

A running, dated record of the iterative development behind OUTRAM PARK's Monte
Carlo criticality path. Kept at paper quality: each entry states what was tried,
the quantitative result (with uncertainty), the diagnosis, and the adjustment
made. The intent is that this file can be lifted, largely as-is, into the
methodology / development section of a publication — the value of a benchmark
result is inseparable from the path taken to it.

> Scope: the `openmc-libs` Monte Carlo transport kernel and the
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

`cargo run --release -p openmc-libs --example godiva_keff` on the ICSBEP
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

This result is encoded as a regression assertion — the LOW and HIGH runs must agree
to within ~2000 pcm and both stay above unity — in
`openmc-libs::physics::keff::tests::godiva_endf_high_fidelity_is_not_data_limited`
(behind the `net-fetch` feature).
