# Non-condensable species and interfacial entropy production (gh:#1064)

> Research, education and V&V only. AI draft, unreviewed.

Part of the RELAP-7 parity epic (gh:#1056). The RELAP-7 Theory Manual
§2.1.1 (Eqs 18, 23), §3.1.5 (Eqs 128–132) and §3.1.8 (Eqs 149–155) is
private, so it is cited by equation number only. Dated 2026-11-01.

## What was built

- **A phasic non-condensable mass balance** in
  `tampines::multiphase_1d::two_fluid::TwoFluid1d`:
  `∂(α_g ρ_g Y A)/∂t + ∂(α_g ρ_g u_g Y A)/∂x = 0` (Eqs 128–132). The
  concentration `c = α_g ρ_g Y` is updated conservatively with the same
  donor-cell gas mass fluxes as the vapour mass equation, `Y` upwinded on the
  flux sign. It takes no part in `Γ`, so `Y` rises where steam condenses.
  API: `set_noncondensable_mass_fraction`,
  `set_inflow_noncondensable_mass_fraction`, `noncondensable_concentration`,
  `noncondensable_mass_fraction`, `noncondensable_inventory`.
  **Added with gh:#1059:** the seven-equation model carries the same
  balance (`c` conserved with the vapour's flux); verified in
  `seven_equation.md` (closed-tube drift 6.6e-16, pulse centroid / `u t` =
  1.000018).
- **The species is a PASSIVE scalar.** The gas equation of state stays
  IAPWS-IF97 steam; the Dalton mixture (steam at its partial pressure plus an
  ideal non-condensable) is not applied, because it would reach every stage of
  the six-equation march (pressure equation, IF97 phase recovery, interface
  temperature). Valid for trace amounts only; stated in the API docs.
- **Interfacial entropy production** (Eq 155),
  `tampines::multiphase_1d::entropy::interfacial_entropy_production`, term by
  term from the equations `TwoFluid1d` solves: heat transfer, phase change, the
  IF97 saturation-consistency residual, drag (friction work at `u_int`) and
  the kinetic energy carried by the transferred mass. The derivation is in
  the module docs.
- **An area-averaged entropy diagnostic** (Eqs 18, 23):
  `TwoFluid1d::entropy_inventory`, `S = Σ V (α_g ρ_g s_g + α_l ρ_l s_l)` with
  IF97 Region 1 / Region 2 entropies at each phase's own `(p, T_k)`.

Searched first: no species field in any 1-D solver; the only
non-condensable in `tampines` is the wet-well component's N2/steam gas space
(gh:#1075), a lumped volume, not a transported field. No entropy-production
check existed.

## Predictions (written into the tests before the first run)

1. Closed vertical pipe: non-condensable inventory drift below `1e-13`;
   `c ≥ 0`; the profile moves; `Y` rises where vapour condenses.
2. Pulse: the centroid moves with the gas, displacement / `∫ ū_g dt` = 1
   within 1 %; mass conserved while the pulse is inside (`1e-6`).
3. Closed adiabatic pipe relaxing by condensation: `S` never falls (beyond
   `1e-12 S` per step) and rises overall.
4. Entropy sweep: heat-transfer and drag terms `≥ 0` everywhere; the
   mass-transfer kinetic term `≥ 0` under the donor-velocity ablation and
   negative somewhere under the mass-weighted `u_int`, but orders of
   magnitude below the heat term; the phase-change term third order and
   small; **the total `≥ 0` at every point**.

## Methodology and results

Tests in `src/multiphase_1d/species_tests.rs` and
`src/multiphase_1d/entropy/tests.rs`.

| Case | Inputs | Measured | Prediction |
|---|---|---|---|
| closed pipe | vertical 1 m, 10 cells, 7 MPa, `α_g` 0.3, liquid 3 K subcooled, `Y` 0.05 in the lower half, 300 × 1 ms | drift **6.0e-16**; min `c` 0; max `|c − c0|` 0.159 kg/m³; gas mass 109.57 → 104.01 kg/m³; bottom `Y` 0.0500 → 0.0528 | met |
| pulse | horizontal 4 m, 80 cells, 7 MPa saturated, `α_g` 0.5, 1 m/s inlet, Gaussian `Y` at 0.7 m, 240 × 2.5 ms | ratio **1.0039**; mass `2.8e-10` | met (third run; see below) |
| entropy inventory | closed 1 m, 4 cells, 7 MPa, `α_g` 0.1, liquid 5 K subcooled, 200 × 0.1 ms | `S` +1.89e-3 J/K of 4113.9; no step fell (worst drop 0) | met |
| entropy sweep | 4032 states × 3 closure sets × 4 options (below) | see the next table | **MISSED** (total) |

**Pulse protocol misses, recorded.** The first run (pulse at 0.5 m in a 2 m
pipe) gained `8.1e-6` of pulse mass: the prescribed-velocity inlet re-injects
the adjacent cell's `Y`, and the Gaussian tail at `x = 0` re-entered. The
second (0.7 m in the same pipe) lost `5.5e-4` through the outlet as the
diffused tail left. The pipe was lengthened to 4 m so that the pulse stays
inside; the `1e-6` criterion was not changed.

### The entropy sweep

States: `p` ∈ {0.5, 2, 7, 15} MPa; `α_g` ∈ {0.01, 0.1, 0.3, 0.6, 0.8,
0.95}; `T_l − T_sat` ∈ {−30, −10, −1, 0, +1, +10, +25} K;
`T_g − T_sat` ∈ {−20, −1, 0, +1, +20, +100} K; `u_l = 1 m/s`,
`u_g − u_l` ∈ {−2, 0, 0.5, 5} m/s. Closure sets: bubbly (1 mm) with the
gh:#1060 regime-map area at `D_h` = 50 mm, the same without it, and the
droplet set (0.1 mm). Options: the gh:#1060 defaults and each ablation. 408
states are outside the property layer's own range (liquid above 623.15 K, or
vapour subcooled beyond the Region-2 equation, at 15 MPa) and are skipped by
construction; the closures refused none of the remaining 10 872 evaluations
per option.

| option | min heat | min drag | min kinetic | negative totals | worst `−σ / (positive terms)` |
|---|---|---|---|---|---|
| defaults (`u_int`, `H_int`) | 0 | 0 | −1.9e3 | **4** | 0.94 |
| without `u_int` | 0 | 0 | 0 | **17** | 1.75 |
| without `H_int` | 0 | 0 | 0 | **4** | 0.94 |
| both ablated | 0 | 0 | 0 | **17** | 1.75 |

Units W/(m³·K). Largest IF97 saturation-consistency term: 1.3e3 W/(m³·K),
against heat terms of 1e5–1e8 at the same states; the count of negative
totals is unchanged without it.

**Interpretation.**

- Heat transfer and drag satisfy the second law at every state, as they must
  with non-negative coefficients.
- The kinetic energy carried by the transferred mass is not sign-definite
  under the mass-weighted `u_int` (algebra in the `entropy` module docs:
  `Γ w² (1 − 2x)/(2T)` at equal temperatures). It went negative as predicted,
  at most 0.77 % of the heat term at the same state. The donor-velocity
  ablation keeps it non-negative.
- **The total is negative at 4 states with the defaults**, all at 15 MPa with
  the vapour 100 K superheated against 10 K subcooled liquid (`α_g` 0.8 and
  0.95). The phase-change term causes it: condensation driven by the
  subcooled liquid removes vapour at `h_g^sat`, well below the superheated
  bulk enthalpy, and so heats the remaining vapour further. That entropy sink
  is of order `Γ c_p ΔT_g² / (2T²)`; the heat-transfer production does not
  cover it once `ΔT_g` is large and `ΔT_l` small. My prediction that the term
  is third order and small held for small departures but was wrong at 100 K
  of superheat.
- Ablating the friction work (`without_interface_velocity`) removes the drag
  dissipation and raises the count to 17. This is one more reason the
  gh:#1060 default is the better one.

The test pins the miss rather than asserting it away: it fixes the exact count
(4 with the defaults) and requires every violation to be a +100 K
vapour-superheat state. If a closure change moves the set, the test fails
and the record has to be revisited.

## What this does not establish

- The species is passive: no partial-pressure effect on condensation. That
  effect is what non-condensables matter for, and it is not modelled.
- The sweep checks the closures, not the stepped fluxes. `TwoFluid1d` makes
  the heat fluxes implicit and may rate-limit `Γ`. Both keep the signs of the
  heat terms and the energy jump, but the stepped values are not swept.
- No experiment is involved; everything here is verification.

## Open items (for the maintainer to file)

- A second-law-consistent interface enthalpy for the losing phase (or a
  documented bound on the superheat for which the closure is used). Without
  it, Eq 155 fails at strong vapour superheat against subcooled liquid.
- A Dalton-mixture gas EOS, so that the species is active.

## How to run

```bash
cargo test --release -p tampines --lib multiphase_1d::species_tests -- --nocapture
cargo test --release -p tampines --lib multiphase_1d::entropy -- --nocapture
```
