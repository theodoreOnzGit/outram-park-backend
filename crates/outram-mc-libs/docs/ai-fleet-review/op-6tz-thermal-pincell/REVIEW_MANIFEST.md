# REVIEW MANIFEST — S(α,β) thermal scattering + thermal LWR pin-cell (op-6tz.12)

**⚠️ AI-GENERATED DRAFT — HUMAN REVIEW REQUIRED per `RESPONSIBLE_USE.md`.**
Everything below was produced by an AI assistant (Claude Opus 4.8) and is
untrusted draft material until a human reviews the physics, the data wiring, and
the measured result. It has passed `cargo build`/`cargo test` but has **not** been
validated against an external benchmark.

- **Date:** 2026-07-15
- **Crate:** `outram-mc-libs` (primary); consumes `njoy-outram-park-fork` (no
  changes made to njoy).
- **Bead:** op-6tz.12 — *outram-mc: S(α,β) thermal scattering in transport
  (thermal-spectrum pincell)*.
- **V&V stage:** Prototype → **Unit Tested / Integrated** (verification only; no
  external benchmark validation yet).

---

## 1. What got wired

The mission was to make the H-in-H₂O S(α,β) thermal-scattering data (already
ported in `njoy-outram-park-fork::thermr`) actually drive the `outram-mc-libs`
transport loop, and turn the previously-`#[ignore]`d thermal LWR pin-cell test
LIVE.

1. **`src/material/thermal.rs` — rewrote the empty `ThermalScattering` stub into a
   real, pre-tabulated data surface.** It is built from the njoy consumer struct
   `IncoherentInelasticScattering` (`from_endf_file`) and **bakes** two grids at
   construction so the transport hot loop stays cheap (the njoy kernel integrates
   the S(α,β) double-differential per call):
   - σ_inel(E) per principal atom on a 200-point log grid `[1e-5, 4] eV`;
   - equiprobable emission tables (16 outgoing energies × 8 cosines, NJOY-typical)
     on a 48-point incident-energy log grid.
   Run-time methods: `inelastic_xs(e)` (linear interp) and `sample(e, seed) ->
   (e_out, mu_lab)` (ACE IFENG=0-style: statistical incident-energy interpolation,
   then one equiprobable E′ bin + one equiprobable **lab-frame** cosine). Cutoff =
   4.0 eV (`DEFAULT_THERMAL_CUTOFF_EV`).

2. **`src/material/nuclide.rs` — hooked the table onto the H-1 nuclide.**
   - New optional field `thermal: Option<ThermalScattering>` (both constructors set
     `None`); builder `Nuclide::with_thermal_scattering(self, ts)`.
   - `xs_at_energy` now: computes the base free-gas/CE `MicroXS`
     (`base_xs_at_energy`, factored out), then **below the cutoff replaces the
     elastic channel with σ_inel(E)** and rebuilds `total` (H₂O has no thermal
     elastic; absorption/fission/inelastic/(n,2n) kept from the base). Consistent by
     construction: `macro_xs_total`/`sample_nuclide`/the reaction partition all read
     `xs_at_energy`, so they see the thermal cross section automatically.
   - New `sample_thermal(e, seed) -> Option<(f64, f64)>` returning a lab-frame
     `(e_out, mu_lab)` (up-scatter allowed).

3. **`src/physics/transport_csg.rs` — added the thermal branch.** In the scatter
   (else) arm, a nuclide with a table below its cutoff samples the bound-atom law
   (`sample_thermal` → `rotate_direction`, no CM transform) instead of the free-gas
   elastic kernel. Fuel/O/clad have no table → unchanged free-gas/CE. Updated the
   module fidelity note (was "No S(α,β) yet").

4. **`src/prelude.rs`** — re-export `ThermalScattering`.
   **`Cargo.toml`** — added `uom` (workspace) **only** for the njoy boundary
   conversion in `thermal.rs`; the transport hot loop stays raw `f64`.

5. **`tests/openmc_notebooks/pincell.rs`** — the thermal pin is now a **LIVE**
   test (`pincell_lwr_thermal_pin_benchmark`, no longer `#[ignore]`): full openmc
   `pincell` geometry (UO₂ r=0.39 / void gap / Zr clad 0.40–0.46 / light water,
   1.26 cm reflective pitch, infinite z) with S(α,β) on H-1. Data-gated on the
   public `tsl-HinH2O.endf` file (see §5).

---

## 2. Build / test output (actual)

```
cargo build -p outram-mc-libs --release      # clean, no warnings
cargo test  -p outram-mc-libs --release --lib
    test result: ok. 39 passed; 0 failed; 0 ignored
cargo test  -p outram-mc-libs --release --test openmc_notebooks
    test result: ok. 8 passed; 0 failed; 18 ignored
    test pincell::pincell_lwr_thermal_pin_benchmark ... ok
```

Thermal pin-cell run (seeded, deterministic; `--nocapture`):

```
[thermal pincell] S(α,β) + nuclide build: 3.83s
[thermal pincell] k_inf = 1.39802 ± 0.00652  (npart=600, 40+60 gen, 7.1s)
```

**Measured result: k_inf = 1.39802 ± 0.00652** (σ = standard error of the mean
over 60 active generations, ≈ 470 pcm). Interpretation: this is squarely inside
the ~1.30–1.45 physical band for a 3% enriched UO₂ light-water pin-cell, so the
thermalization is behaving physically. It is **not** claimed as benchmark-accurate
(no external reference run was performed; see §4/§6).

**Thermal-branch evidence (qualitative).** A with-vs-without-S(α,β) contrast run
(throwaway, not committed): the **with** case terminates normally in ~7 s; the
**without** case (free-gas H at rest, no up-scatter) does **not** complete in
2 min — thermal neutrons pile up at ultra-low energy and histories only die via
the `MAX_EVENTS` cap. This directly confirms (a) the thermal branch changes
behavior, and (b) S(α,β) is what lets thermal histories terminate efficiently.

---

## 3. Provenance

- **Transport/geometry**: ported in structure from OpenMC C++
  (`/home/teddy0/Documents/research/openmc/`, MIT) — `src/physics.cpp`,
  `src/geometry.cpp`, `src/thermal.cpp` (ACE `ThermalData::sample`, IFENG=0).
- **S(α,β) data kernel**: `njoy-outram-park-fork::thermr` — a port of NJOY2016
  (release 2016.79, modified-BSD, GPL-compatible) `thermr.f90`.
- **Nuclear data**: ENDF/B-VIII.0 `tsl-HinH2O.endf` (public IAEA/NNDC ENDF/B
  release) for H-in-H₂O S(α,β) at MAT 1, nearest tabulated T to 293.6 K; embedded
  CORE WMP + fast MGXS (from `njoy-outram-park-fork`) for U-235/238, O-16, Zr.
- **Geometry/material spec**: openmc `pincell.ipynb`
  (openmc-notebooks @ `cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT): UO₂
  (3% U235 / 97% U238 / 2 O, 10.0 g/cm³), natural Zr clad (6.6 g/cm³), unborated
  light water (1.0 g/cm³), fuel r=0.39, clad 0.40–0.46, 1.26 cm reflective pitch.

---

## 4. Assumptions & known approximations (NOT benchmark-validated)

1. **Thermal cutoff = 4.0 eV**, fixed. The njoy `inelastic_xs` uses an SCT tail
   that never returns exactly zero (it relaxes to the free-atom limit), so a hard
   cutoff is imposed. By 4 eV σ_inel ≈ σ_free and up-scatter is negligible, so the
   join to free-gas WMP elastic is smooth — but the exact cutoff is a modeling
   choice, not read from the data.
2. **O-16 and fuel are free-gas with the target at rest** below the cutoff (0 K
   free gas): they can only down-scatter. This is per the port's documented scope
   (O is nearly free at thermal). Physically the Maxwellian up-scatter is supplied
   by H via S(α,β); since H dominates thermal collisions in water this is a
   reasonable approximation, but it is an approximation. **No free-gas thermal
   target-motion treatment for O/fuel is implemented.**
3. **LOW-tier thermal data for U/O/Zr**: WMP below `e_max` (valid into the thermal
   range) + a constant-ν̄ stopgap in the resonance range (`nubar_for`, U235 = 2.44
   ≈ the thermal value). Adequate for a thermal pin but not the full ENDF fidelity.
4. **Unborated water** (the canonical pincell notebook is unborated; a borated
   variant would lower k_inf). No boron added.
5. **Emission grid resolution** (48 incident energies, 16×8 bins) is a
   performance/accuracy trade — not convergence-studied here.
6. **σ over 60 active generations ≈ 470 pcm** — a verification-grade uncertainty,
   not a converged benchmark uncertainty.

---

## 5. "Additional njoy-data needed" assessment (user's standing ask)

**Verdict: for the thermal UO₂ pin-cell, NO additional njoy data wiring was
needed beyond the existing surfaces.** Detail:

| Nuclide | Thermal-range coverage in the LOW tier | Adequate for this pin? |
|---|---|---|
| H-1 | WMP (covers thermal) **+ new H-in-H₂O S(α,β)** from `thermr::scattering` | Yes — this was the wiring gap; now closed. |
| O-16 | CORE WMP (38 windows; window-0 clamp gives sensible thermal σ) — free-gas | Yes (O nearly free at thermal; documented approximation). |
| U-235 | CORE WMP below `e_max` covers the thermal + resonance range; ν̄ = 2.44 stopgap | Yes for a first thermal k_inf. |
| U-238 | CORE WMP covers thermal + resonances (incl. the big capture resonances) | Yes. |
| Zr 90/91/92/94/96 | all five in CORE WMP | Yes. |

So the **only** genuinely missing piece was the S(α,β) consumer wiring on the
outram-mc side — the njoy THERMR physics and the `IncoherentInelasticScattering`
surface were already present and sufficient. **No `njoy-outram-park-fork` source
was modified.**

**Where more njoy data *would* raise fidelity (future, not blocking):**
- Pointwise CE thermal-range XS for U-235/238 and O-16 (HIGH tier `from_endf`,
  behind `net-fetch`) instead of WMP+stopgap-ν̄ — for benchmark-accuracy.
- A **free-gas thermal target-motion** treatment (or S(α,β)-free thermal kernel)
  for O-16/fuel below the cutoff, for correct up-scatter off non-H nuclei.
- Per-nuclide energy-dependent ν̄/χ baked into the CORE tier (already flagged in
  `docs/keff-doppler-roadmap.md`).
- A **tsl acquisition path** in njoy (download/cache the `tsl-*` file) so the LIVE
  test is not gated on a local absolute path (see below).

---

## 6. Thermal test: LIVE vs IGNORED

**LIVE** — `pincell_lwr_thermal_pin_benchmark` is no longer `#[ignore]`d and
asserts a real, stationary k_inf ± σ in a broad physical band `[0.9, 1.7]`.

**Caveat (honest): it is DATA-GATED, not fully portable.** The ENDF/B-VIII.0
`tsl-HinH2O.endf` file is public but large and **not vendored in the repo**
(per `DATA_POLICY.md` — public data referenced by path). The test locates it via
`OUTRAM_TSL_HINH2O` or a known local path; **if absent it prints a clear `SKIP`
and returns without asserting the physics.** On a machine without the file the
test therefore passes trivially (a documented soft-skip, not a fabricated green —
the S(α,β) unit tests in `thermal.rs` still run unconditionally). On this
harness the file *is* present, so the physics ran and produced the number above.
A human reviewer should decide whether to (a) accept the data-gated pattern, or
(b) vendor a small `tsl` subset / add a njoy acquire path so CI runs it too.

---

## 7. Human-verify list (top asks)

1. **Thermal-scattering correctness (TOP).** Verify `thermal.rs` sampling against
   OpenMC `src/thermal.cpp` `ThermalData::sample` (IFENG=0): the equiprobable
   E′-bin + cosine draw, the incident-energy statistical interpolation, and that
   the njoy `emission` bins are consumed in the intended (lab-frame) convention.
2. **Transport branch correctness (TOP).** Confirm that replacing the elastic
   channel with σ_inel below the cutoff (and *only* below it) is right, that the
   reaction partition stays consistent (total rebuilt from
   absorption+inelastic+n2n+σ_inel), and that no double-counting of scattering
   occurs at the cutoff boundary.
3. **The k_inf number.** Cross-check 1.398 against a real OpenMC pincell run with
   the same materials/geometry (the actual validation step — not done here).
4. **Cutoff choice (4 eV)** and the free-gas-O approximation below it — are these
   acceptable for the intended educational/V&V use, or does O need thermal
   target-motion?
5. **Determinism / convergence.** The result is seed-deterministic; a human may
   want a convergence study (particles, generations, emission-grid resolution).
6. **Data-gating policy** (§6) — accept soft-skip vs make CI-portable.

---

## 8. Files changed

- `crates/outram-mc-libs/src/material/thermal.rs` (rewrite: stub → real)
- `crates/outram-mc-libs/src/material/nuclide.rs` (thermal field + builder + XS
  override + `sample_thermal`)
- `crates/outram-mc-libs/src/physics/transport_csg.rs` (thermal scatter branch +
  doc)
- `crates/outram-mc-libs/src/prelude.rs` (`ThermalScattering` re-export)
- `crates/outram-mc-libs/Cargo.toml` (+`uom`, boundary-only)
- `crates/outram-mc-libs/tests/openmc_notebooks/pincell.rs` (thermal pin LIVE)
- `crates/outram-mc-libs/docs/ai-fleet-review/op-6tz-thermal-pincell/REVIEW_MANIFEST.md` (this file)
- `Cargo.lock` (uom already in tree via njoy; lock refresh)

No changes to `njoy-outram-park-fork`.
