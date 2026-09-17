# REVIEW MANIFEST — THERMR H-in-H₂O S(α,β) thermal scattering (op-cjw.19)

> **⚠️ AI-GENERATED DRAFT — HUMAN REVIEW REQUIRED per `RESPONSIBLE_USE.md`.**
> All code and numbers below were produced by an AI agent and are **untrusted
> draft material** until a human reviews the physics, the provenance, and the
> V&V. Not for nuclear facility operation, reactor control, licensing, or any
> safety-critical use.

- **Bead:** op-cjw.19 (parent epic op-cjw; unblocks op-6tz.12 thermal pincell).
- **Date:** 2026-07-15.
- **Scope touched:** `crates/njoy-outram-park-fork/**` only.
- **Model:** Opus 4.8 (lead, no subagents — the physics is tightly coupled
  across `mf7.rs` + `inelastic.rs`, so it was kept in one hand).

## Goal

Complete the incoherent-inelastic S(α,β) → σ(E) path for **H in light water**
at **293.6 K**, from the ENDF/B-VIII.0 `tsl-HinH2O.endf` evaluation, and expose
it via a data surface `outram-mc-libs` can consume for a THERMAL pincell.

## Provenance

- **Nuclear data:** ENDF/B-VIII.0 thermal scattering sublibrary,
  `tsl-HinH2O.endf` (17.4 MB), read from
  `/home/teddy0/Documents/research/ENDF-B-VIII.0/thermal_scatt/tsl-HinH2O.endf`.
  MAT = 1. Open-source (ENDF/B-VIII.0, 2018), permitted per `DATA_POLICY.md`.
  Parsed parameters: `LAT=1`, `LASYM=0`, `B(1)=40.872`, `A=B(3)=0.99917`,
  `natom=B(6)=2`, secondary `B(7)=1` (free-gas oxygen — see assumptions),
  317 β values, 222 α values, S(α,β) tabulated at 18 temperatures
  (base T₀=283.6 K; 293.6 K is the first extra temperature).
  **The 17.4 MB file is NOT checked in** (existing tsl fixtures are ≤2 MB).
- **Algorithm:** faithful port of **NJOY2016 release 2016.79, commit
  `ac5adf5f33d893e42f2eed7fb286b0d51c7580da`**, `src/thermr.f90`
  (`calcem` reader lines 1656–1834; `sig` double-differential + SCT branch,
  lines 2482–2615). Kept line-traceable to those subroutines.
- License: crate stays `GPL-3.0-only` (derivative of modified-BSD NJOY2016);
  `LICENSE.njoy` + `NOTICE` unchanged.

## Files changed

| File | Change |
|---|---|
| `src/thermr/mf7.rs` | `IncoherentInelastic` gains a `teff_table` field (principal-scatterer effective temperature). New `parse_mf7_at_temperature(tape, mat, target_k)` selects the S(α,β) tables at the **nearest tabulated temperature** (fixes a silent base-T-only bug). `parse_inelastic` now retains the selected temperature's S and reads the trailing effective-temperature TAB1. `parse_mf7` unchanged (base T₀) for back-compat. |
| `src/thermr/inelastic.rs` | New `teff_ev(temp_k)` (interpolate `T_eff`, K→eV). New `sct_double_differential(...)` — the **short-collision-time tail** (port of `sig` label 170). `double_differential` now routes out-of-table (α or β beyond the grid) and floored-corner lookups through the SCT kernel instead of returning 0 — this is what restores the free-atom limit at high E. Added `interp_linear` helper. |
| `src/thermr/scattering.rs` | **NEW.** `IncoherentInelasticScattering` — the consumer surface (see below) + `ThermalEmissionBin`. |
| `src/thermr/mod.rs` | Wires `pub mod scattering;` + re-exports; status docs updated. |
| `tests/thermal_h2o_sab.rs` | **NEW.** 5 V&V tests (below). Reads the ENDF file from `$HINH2O_TSL` or the default path; **skips (passes) if absent** so CI stays green. |
| `docs/ai-fleet-review/op-cjw-thermr-h2o/REVIEW_MANIFEST.md` | This file. |

No other crates touched. `main` untouched.

## Consumer interface for `outram-mc-libs`

The thermal pincell fleet should call **`thermr::scattering::IncoherentInelasticScattering`**:

```rust
use njoy_outram_park_fork::thermr::scattering::IncoherentInelasticScattering;
use njoy_outram_park_fork::units::{NeutronEnergy, Temperature};
use uom::si::{energy::electronvolt, thermodynamic_temperature::kelvin, area::barn};

// Load once per material + temperature (MAT 1 for tsl-HinH2O).
let t = Temperature::new::<kelvin>(293.6);
let sab = IncoherentInelasticScattering::from_endf_file("tsl-HinH2O.endf", 1, t)?;

// Confirm the tabulated temperature actually used is close enough.
assert!((sab.selected_temperature().get::<kelvin>() - 293.6).abs() < 0.1);

// σ_inel PER PRINCIPAL ATOM (per H). Multiply by the H number density.
let e  = NeutronEnergy::new::<electronvolt>(0.0253);
let xs = sab.inelastic_xs(e);                 // -> CrossSection (uom Area)

// Secondary energy/angle sampling: 16 equiprobable E', 8 equiprobable cosines.
let bins = sab.emission(e, 16, 8);            // -> Vec<ThermalEmissionBin>
// sample: pick a bin uniformly (1/16), then a cosine uniformly (1/8).
```

Key contract points the MC fleet must respect:

- **Cross sections are per principal atom (per H).** For H₂O multiply by the H
  number density (2 per molecule). `principal_atom_count()` returns 2.
- Only the **incoherent-inelastic** channel is here. Light water has **no
  thermal elastic**, so that is complete for H₂O. Oxygen is *not* in this data
  (see assumptions) — treat O with its ordinary free-gas elastic cross section.
- Helper queries: `free_cross_section()` (20.436 b/H, high-E limit),
  `bound_cross_section()` (81.8 b/H, E→0 static limit),
  `effective_temperature()` (1194 K), `mass_ratio()`, `kernel()` for the raw
  double-differential / tabulated grid.

## Assumptions & limitations (human-review the starred items)

1. **★ Secondary scatterer (oxygen) is excluded** — matches NJOY. For H-in-H₂O
   `B(7)=1` (free-gas option), and NJOY `calcem` only folds a secondary atom
   into the S(α,β) treatment when `B(7)=0` (SCT). So oxygen contributes nothing
   here and must be handled by the MC code as ordinary free-gas O. *If a future
   evaluation sets `B(7)=0`, the secondary SCT term is currently skipped* (its
   `T_eff2` table is not parsed) — flagged in `sct_double_differential` docs and
   filed as a follow-up bead.
2. **★ Temperature selection is nearest-tabulated, not interpolated.** 293.6 K
   is tabulated exactly, so this is loss-less for the target case; a request
   between tabulated points snaps to the nearest (report via
   `selected_temperature()`). No S(α,β) temperature interpolation.
3. **IFENG=0 (equiprobable) emission only** — pre-existing THERMR/ACE caveat,
   unchanged. Skewed/continuous (IFENG=1/2) not ported.
4. **Fixed-grid quadrature** over μ (200 pts) and a β-derived E′ grid — adequate
   for σ(E) at the ~1% level shown; NJOY's adaptive refinement and the liquid
   small-α (`cliq`) correction are not ported (documented in `inelastic.rs`).
5. **`natom` is a single scalar** (nmix=1) — no multi-population mixing.

## V&V — methodology and measured results (2026-07-15, ENDF/B-VIII.0)

Reproduce: `HINH2O_TSL=<path> cargo test -p njoy-outram-park-fork --release --test thermal_h2o_sab -- --nocapture`
(full methodology in the test-file `//!` doc). All 5 pass.

| # | Check | Reference | Tolerance | Measured | Verdict |
|---|---|---|---|---|---|
| 1 | Free-atom high-E limit | σ_free = B(1)/2 = 20.436 b/H | ≤5 % at 8 eV, monotone 1→8 eV | 20.707 b/H at 8 eV (**+1.33 %**); 21.713→20.951→20.707 over 1→4→8 eV | ✅ |
| 2 | Thermal σ | H₂O bound σ_s ≈ 103 b/molecule @ 0.0253 eV (literature) | 90–115 b | 52.10 b/H → **104.2 b/molecule** (+1.2 %) | ✅ |
| 3 | Detailed balance of d²σ | `(E'/E)·exp(−(E'−E)/kT)` (analytic) | <1 % rel | ratio 1.322697 vs 1.322697, **rel 1.7e-16 (machine-exact)** | ✅ |
| 4 | Effective temperature | T_eff > T, H-in-H₂O band ~1100–1300 K | 293.6 < T_eff < 2000 K | **1194.3 K** | ✅ |
| 5 | Temperature selection | 293.6 K tabulated | <0.1 K | selected **293.600 K** | ✅ |

**Interpretation.** Checks 1 and 3 together exercise the full S(α,β)
integration. Before this work, σ_inel(8 eV) was ~2 b (SCT tail missing) and the
kernel used the 283.6 K S tables for a 293.6 K request; both are fixed. These
are *limiting/analytic* checks — **not** a pointwise comparison against an NJOY
ACE/GENDF oracle (none was run).

## Human-verify checklist (top ask first)

1. **★★ S(α,β) integration correctness** — the top ask. Confirm `sigma_ep_profile`
   (the β-derived E′ grid) + the μ-quadrature in `inelastic.rs` capture the
   double-differential correctly, including the SCT tail near the quasi-elastic
   peak. The thermal value depends on the upscatter (E′>E) branch; sanity-check
   it against a trusted S(α,β) integrator if available.
2. **★ SCT formula fidelity** — verify `sct_double_differential` against
   `thermr.f90` label 170 (lines 2569–2598): the `(α−|β|)²·T/(4α·T_eff)` +
   `(|β|+β)/2` argument, the `√(4π·α·T_eff/T)` normalization, and that physical
   (un-scaled) α,β are used.
3. **★ Ideally reproduce an NJOY ACE/GENDF oracle** for tsl-HinH2O at 293.6 K
   and compare σ_inel(E) pointwise — the analytic checks here don't cover the
   mid-energy shape (0.05–1 eV) quantitatively.
4. Confirm the "per principal atom" convention is what the MC fleet expects, and
   that excluding oxygen from this data (handled as free-gas) is correct for the
   pincell.
5. Confirm nearest-temperature selection (no interpolation) is acceptable for
   the intended temperatures.

## Follow-ups filed

- Secondary-scatterer SCT term (`B(7)=0` evaluations) — parse `T_eff2` and add
  the second SCT term (currently skipped).
- NJOY ACE/GENDF pointwise oracle comparison for tsl-HinH2O.
- S(α,β) temperature interpolation (vs nearest-tabulated).
