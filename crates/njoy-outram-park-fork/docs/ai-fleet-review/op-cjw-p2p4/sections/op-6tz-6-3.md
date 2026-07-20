# op-6tz.6.3 — mgxs notebooks: flux-solved / self-shielded MGXS

**⚠️ AI-GENERATED DRAFT — HUMAN REVIEW REQUIRED per `RESPONSIBLE_USE.md`**

**Status: PARTIAL (advanced this pass; bead stays OPEN).** Done by the lead agent
directly (not a subagent), building on op-cjw.15's newly-ported GROUPR vector
group-average engine.

## What changed

- **`tests/openmc_notebooks_data/mgxs_part_i.rs`** — added one LIVE test,
  `groupr_engine_vector_group_average`, and rescoped the previously-`#[ignore]`
  `flux_weighted_self_shielded_mgxs` (updated its ignore reason to reflect that
  the *vector* path now exists but the *matrix*/self-shielded path does not). No
  other file changed. `mgxs_part_ii.rs` / `mgxs_part_iii.rs` were not touched.

## What is DONE (live, verified)

The GROUPR **vector** group-average engine from op-cjw.15
(`groupr::panel::group_average_vector`) is now exercised on a real evaluation:
it group-averages the RECONR-reconstructed U-235 total cross section over the
notebook's 2-group structure under a 1/E weight, and is cross-checked against the
independent lightweight `Mgxs::collapse` primitive.

### Methodology
- Data: U-235 ENDF/B-VIII.0 (MAT 9228), RECONR at 0.001 tol / 0 K.
- Grid: 3000-point log grid over `[1e-3, 2e7]` eV; group edges
  `[1e-3, 0.625, 2e7]` eV (thermal edge = reconstructed-grid floor; the 1/E
  weight and the σ table are undefined at exactly 0 eV).
- Pass criterion: both engines finite/non-negative; the two group-average
  implementations agree to `< 0.5%`; thermal-group total `≥ 5×` fast-group total
  (1/v absorber signature).

### Results (2026-07-15)
- GROUPR panel engine σ_t = `[1071.152, 35.1865]` barn (thermal, fast).
- `Mgxs::collapse` σ_t     = `[1071.448, 35.1924]` barn.
- Relative difference: `2.8e-4` (thermal), `1.7e-4` (fast) — agreement under
  0.03%. Both are trapezoid reductions of the same linearly-interpolated σ over
  the same union grid; the residual is only the two engines' internal
  panel-refinement.
- Interpretation: the two independently-implemented fixed-spectrum group-average
  paths corroborate each other on a real nuclide. This is a **verification**
  (implemented-consistently) result, NOT a validation against a real NJOY GROUPR
  GENDF tape — see the gap + human-verify note.

## What is still PARTIAL / NotPorted (honest gap — bead stays OPEN)

The notebook's actual deliverable is *flux-solved, self-shielded* MGXS with a
group-to-group **scatter matrix** and group **Chi**. Those are NOT delivered:

- **Self-shielding** (dilution / Bondarenko narrow-resonance weighting) — the
  collapse here uses a fixed 1/E spectrum, not a self-shielded flux.
- **Scatter matrix** (group-to-group) — needs the GROUPR **matrix** path
  (`cm2lab` kinematics + File-6 feeders `getmf6`/`getff`/`getyld`), which
  op-cjw.15 leaves `NotPorted`.
- **Group Chi** (fission spectrum) — same matrix path.
- The alternative route is transport-tally MGXS from `outram-mc-libs`
  (`ScatterMatrixXS`, `mgxs.run()`), which is a different crate.

`flux_weighted_self_shielded_mgxs` remains `#[ignore]` with a reason naming these.

## Human-verify asks (top)

1. **Golden-file validation:** the GROUPR vector engine agrees with our own
   collapse primitive, but neither has been checked against a real NJOY GROUPR
   GENDF tape (e.g. a coarse-group U-235/U-238 capture vector). That is the
   trust gate before this is more than "verification".
2. Confirm the thermal-edge choice (1e-3 eV vs the notebook's 0 eV) is acceptable
   for the intended comparison — below 1e-3 eV there is no reconstructed data.

## Build/test (actual)
- `cargo build -p njoy-outram-park-fork --release` — clean.
- `scripts/test.sh --test openmc_notebooks_data` → all tests pass, 0 failed;
  `groupr_engine_vector_group_average` LIVE and green;
  `flux_weighted_self_shielded_mgxs` correctly `#[ignore]`.
