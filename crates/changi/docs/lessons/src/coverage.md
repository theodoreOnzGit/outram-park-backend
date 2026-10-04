# Coverage table

> **Review status:** AI-assisted first draft, 2026-10-04, not yet reviewed by
> a human. Built from commit
> [`@@COMMIT_SHORT@@`](https://github.com/theodoreOnzGit/outram-park-backend/commit/@@COMMIT@@).

Every public module of `changi` (bar `flexpart`, deferred), `buangkok` and
`sembawang`, mapped to the page that teaches it, the code walk that reaches
it, and the record of how it was checked. **A blank V&V cell means there is no
check**, and the gap is said on the page. "c2c" is code-to-code against the
upstream code the module was ported from: verification of the translation,
never validation.

## changi

| Module | Core rung | Extended page | Code walk | V&V record |
|---|---|---|---|---|
| `puff::stability` | [2](./rungs/02-sigmas.md) | [puff internals](./puff.md) | rung 2: example → `stability_class` | c2c, 1 539 cases exact, `docs/puff-code-to-code.md` (2026-09-21) |
| `puff::dispersion` | [2](./rungs/02-sigmas.md) | [puff internals](./puff.md) | rung 2: example → `pasquill_gifford_sigmas` | c2c, 234 cases bit-exact; sigma-set comparison in `buangkok/tests/changi_puff_train_vs_plume.rs` (reported) |
| `puff::concentration` | [4](./rungs/04-puffs.md) | [puff internals](./puff.md) | rung 4: example → `gaussian_puff_concentration` | c2c `gpuff` 6 720 cases ≤ 6.3e-16; mass conservation `tests/puff_mass_conservation.rs` (2026-10-04) |
| `puff::simulate` | [4](./rungs/04-puffs.md) | [puff train](./puff-train.md) | rung 4: `simulate_sensor_mode` → `advect`; tree | c2c sensor 130 / grid 720 cases ≤ 4.3e-16; veering-wind unit test; #380 Gate 2 |
| `puff::wind` | [4](./rungs/04-puffs.md) | — | rung 4 tree | c2c (via `simulate_*`) |
| `puff::climatology` | [4](./rungs/04-puffs.md) | — | rung 2/4 example tree | none: illustrative data, provenance in `docs/References.md` |
| `puff::wgsl` | — | [map field](./ext/map-field.md) | `field_timing` → `contribution` | timing only (`examples/field_timing.rs`, 2026-09-24); f32 picture, not a quoted number |
| `activity::chi_over_q` | [5](./rungs/05-deposition.md) | [activity](./activity.md) | rung 5: example → `dilution_factors`, → `DilutionFactors::dilution` | #380 Gate 1 ≤ 3.9e-16 (84 cases), Gate 2; `docs/activity-consistency-checks.md` |
| `activity::decay_transfer` | [5](./rungs/05-deposition.md) | [activity](./activity.md) | rung 5 tree | unit tests; Gate 1 (Kr-89-like case) |
| `activity::deposition` | [5](./rungs/05-deposition.md) | [activity](./activity.md) | rung 5: example → `dry_deposition` | consistency only; velocities uncited by design; no wet, no depletion (gap issue on rung 5) |
| `activity::survey` | [5](./rungs/05-deposition.md) | [activity](./activity.md) | rung 5 tree | consistency only |
| `activity::source` | [5](./rungs/05-deposition.md), [7](./rungs/07-capstone.md) | [activity](./activity.md) | rung 5 tree | `SourceTerm::validate` unit tests; `sembawang/tests/chain_handoff.rs` |
| `activity::units` | [5](./rungs/05-deposition.md) | — | rung 5 tree | type-level; `uom` round trip noted in `docs/puff-code-to-code.md` |
| `activity::{inventory, fuel_release, primary_helium, airborne_release, accident_airborne_release}` | [6](./rungs/06-dose.md), [7](./rungs/07-capstone.md) | [published tables](./ext/published-tables.md) | rung 7 tree | stored published data; Liu & Cao cross-check (gh:#379) |
| `flexpart` (34 modules) | side path | [random walk (coming)](./rungs/side-random-walk.md) | — (deferred, gh:#410) | Fortran c2c, double and single precision, `docs/flexpart-code-to-code.md`; stochastic 30/30 within 1 sigma |

## buangkok

| Module | Core rung | Extended page | Code walk | V&V record |
|---|---|---|---|---|
| `pydoseia::dispersion` | [1](./rungs/01-plume.md), [2](./rungs/02-sigmas.md) | [plume internals](./plume.md), [met processing](./ext/met-processing.md) | rung 1: example → `master_equation_single_plume`; rung 2 → `sigma_z` | c2c exact (`docs/pydoseia-code-to-code.md`, 2026-09-28); flux conservation `tests/plume_mass_flux_conservation.rs` (2026-10-04) |
| `pydoseia::met` | (2) | [met processing](./ext/met-processing.md) | driver → `dilution_long_term_with_met` | c2c exact on synthetic met data; D1 corrected variant unverified |
| `pydoseia::plume_rise` | [3](./rungs/03-rise-wake.md) | — | rung 3: example → `plume_rise_neutral_unstable`, → `building_wake_gifford` | c2c (faithful functions, 62 cases exact); corrected D5/D6 and the formulas themselves **unchecked** against their sources (gap issue on rung 3) |
| `pydoseia::dose` | [5](./rungs/05-deposition.md), [6](./rungs/06-dose.md) | — | rung 6: driver → `inhalation_dose`, `submersion_dose`, `ground_shine_dose` | c2c exact, 128 cases per pathway |
| `pydoseia::dcf` | [6](./rungs/06-dose.md) | [driver](./ext/driver-and-integrators.md) | rung 6 tree | c2c exact on synthetic tables |
| `pydoseia::dcf_screening` | — | [driver](./ext/driver-and-integrators.md) | rung 6 tree | c2c exact (`dcf_screening`, `driver_dcf_report`) |
| `pydoseia::plume_shine` | [6](./rungs/06-dose.md) | [integrators](./ext/driver-and-integrators.md) | rung 6: `per_class` → `petir_tplquad` | c2c exact on synthetic photon data; petir vs QUADPACK 120/120 bit-identical; **no real photon data ships** |
| `pydoseia::quadpack` | — | [integrators](./ext/driver-and-integrators.md) | integrators: `line_integral` → `petir_tplquad` | c2c against SciPy (`quadpack`, `tplquad` groups) |
| `pydoseia::ingestion` | [6](./rungs/06-dose.md) | — | rung 6: `ingestion_dose_per_nuclide` tree | c2c exact (faithful driver, 96 cases); corrected driver a divergence; **no transfer factors ship** |
| `pydoseia::nuclide` | [6](./rungs/06-dose.md) | [driver](./ext/driver-and-integrators.md) | rung 6 tree | c2c exact (`half_life`) |
| `pydoseia::{config, assessment}` | (6) | [driver](./ext/driver-and-integrators.md) | driver → `pathway_doses` | c2c against upstream's joblib run (`driver`, `driver_summary`, `boundary_totals_text`) |
| `pydoseia::units` | (6) | [driver](./ext/driver-and-integrators.md) | — | type-level |
| `coefficients` | [6](./rungs/06-dose.md), [7](./rungs/07-capstone.md) | — | rung 7 tree | provenance in `docs/References.md`; Ba-137m branching vs ENDF/B-VIII.0 +0.32 % (reported) |
| `published` | [6](./rungs/06-dose.md) | [published tables](./ext/published-tables.md) | — | stored data; `tests/liu_cao_external_dose_cross_check.rs` (consistent, not verified) |

## sembawang

| Module | Core rung | Extended page | Code walk | V&V record |
|---|---|---|---|---|
| `scenario` | [7](./rungs/07-capstone.md) | [sembawang](./ext/sembawang.md) | rung 7 tree | unit tests |
| `inventory` | [7](./rungs/07-capstone.md) | [sembawang](./ext/sembawang.md) | rung 7 tree | unit tests |
| `accident::release` | [7](./rungs/07-capstone.md) | [sembawang](./ext/sembawang.md) | rung 7: example → `accident_release_with_venting` | `tests/accident_release.rs`, `tests/normal_operation_pools_code_to_code.rs` (pools vs upstream); release physics is `boon-lay`'s, c2c on that track |
| `accident::venting` | [7](./rungs/07-capstone.md) | [sembawang](./ext/sembawang.md) | sembawang: `accident_release_with_venting` tree | `tests/venting_modes.rs` |
| `chain` | (7) | [sembawang](./ext/sembawang.md) | — | `tests/chain_handoff.rs` (consistency) |
| `htr10` | [7](./rungs/07-capstone.md) | [sembawang](./ext/sembawang.md) | rung 7 tree | unit tests; `verification_and_validation/htr10-dlofc-panama-triso-atops.md` |
| `units`, `error` | (7) | [sembawang](./ext/sembawang.md) | — | unit tests; `tests/nuclide_names_code_to_code.rs` |
| `lwr_comparison` | (7) | [sembawang](./ext/sembawang.md) | — | example records only; not quoted in this book |
| `ap1000_ted` | — | [sembawang](./ext/sembawang.md) | — | a digitised published curve; not a check |
