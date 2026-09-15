# HTR-10 — 10 MWth pebble-bed high-temperature gas-cooled reactor

Scoping document for an offline digital-twin simulator of the HTR-10, built in
`crates/outram-park-digital-twin-engine` with its coupled steam-generator
secondary loop.

> **Intended use.** Education, research, capability building, and V&V only. This
> is an offline demonstration with no connection to any operational system. See
> `RESPONSIBLE_USE.md`.
>
> **Status of this document.** The capability audit was originally performed
> 2026-08-06 and **fully re-verified against the code on 2026-08-12**. Every
> row of the HAVE, SCAFFOLD and MISSING tables was re-read against the source
> on that date and carries an explicit marker: **Re-verified 2026-08-12**
> (claim and details both still hold), **Re-verified, details corrected**
> (claim holds, paths/line numbers/counts were stale and have been fixed), or
> **Corrected 2026-08-12** (the claim was false and the true position is now
> stated). Anything that could not be checked is marked **Not re-checked**
> with the reason.
>
> **Two exceptions to the re-verification, stated plainly:**
>
> 1. **Validation source identifiers remain deliberately unverified** — see
>    [Open validation data](#4-open-validation-data). Nothing in section 4 was
>    confirmed against an actual document, and none of it should be read as
>    checked.
> 2. **Test *pass* status was not re-run for `htgr_sim_v1` and `fhr_sim_v2`.**
>    Both examples were being edited by other agents while this audit ran, so
>    their working-tree state was transient. Test *counts* below are measured
>    from `git show HEAD:` and are reliable; whether they currently pass is
>    **not re-checked**.
>
> Six claims were found **false** and are corrected below: the decay-heat
> "self-flagged as suspect" claim, the "no graphite anywhere" gap, the radial
> pebble conduction gap, the graphite/moderator feedback-channel gap, the
> "trustworthy decay heat" gap, and "nothing HTR-10-specific is in this
> repository". Several more were substantively right but had rotted file paths,
> line numbers or counts — see the pattern note in
> [section 3](#3-capability-audit).
>
> **Sibling documents — spot-checked, NOT fully audited, and NOT corrected.**
> The findings below are the ones personally verified while checking this
> document; **neither sibling received a systematic audit**, so absence from
> this list is not evidence of accuracy.
>
> - `htr10-plant-data.md:422`, `:426`, `:725` record the KTA 3102.3
>   **pressure-drop assembly equation** as too OCR-degraded to recover and list
>   it as outstanding. `crates/outram-park-digital-twin-engine/src/htr10/kta.rs`
>   implements that correlation and is gold-gated against the VTB worked example
>   (3493.17 vs 3493 Pa/m). **Stale.**
> - `htr10-plant-data.md:663-665` still presents the 3.0 MPa primary pressure as
>   a correction to be applied to *this* document. It has been applied here
>   (see the blockquote at the end of section 3) — a cross-reference that has
>   gone stale, not a false statement.
> - `htr10-neutronics.md:662`, `:672`, `:853` treat the `outram-mc-libs` RNG
>   defects `op-rbo` and `op-jis` as open, with `:853` a priority-0 "close the
>   two RNG defects" row. **Both were fixed** — commits `9f4ff6d470` and
>   `e71f1f97fa`, both dated 2026-08-06. Verified by reading `rng/lcg.rs:233`
>   and `:111-121`.
> - `htr10-neutronics.md:257` quotes `outram-mc-libs/src/material/thermal.rs:24-26`
>   as saying graphite coherent/incoherent-elastic scattering "is deliberately
>   not wired here yet". **That text is no longer at those lines**; the module
>   now documents all three ENDF MF=7 channels including coherent elastic for
>   graphite (commit `67ebcd6ca5`, 2026-08-11). The retracted sentence had
>   also survived in the generated mirror,
>   `crates/outram-mc-libs/docs/api.md:7704` (that file has since been renamed
>   `outram-mc-libs-api.md`) — **resolved 2026-08-17**, incidentally, by an
>   unrelated `kovan api-docs --all` regeneration for the `<crate>-api.md`
>   naming convention (op-w5ry). Verified absent from the current mirror.
> - `htr10-neutronics.md:168` states `src/htr10/neutronics.rs` holds **45**
>   published values; the table immediately below it (`:172-178`) has rows
>   summing to **89** (31 + 7 + 14 + 7 + 16 + 6 + 6 + 2). One of the two numbers
>   is wrong; **which one was not determined.**
>
> Neither sibling's *literature* content was audited at all.

Relates to the existing bead `op-wqk.9` (`htgr_sim_v1`) and its children.
**Updated 2026-08-12:** the active epic is now **`op-jyyp`** ("HTR-10 pebble-bed
simulator — retarget `htgr_sim_v1` from prismatic to HTR-10", 20 items),
created 2026-08-10 and carrying the maintainer decision of 2026-08-11 that
`htgr_sim_v1` *is* the HTR-10 simulator. The `op-wqk.9.*` children predate it
and are stale — see [Bead accuracy](#bead-accuracy--three-of-four-children-are-stale).

## 1. Framing correction — read this first

> **This section is OBSOLETE as of 2026-08-12 and is kept only so the change is
> traceable.** Everything below the rule was true when written and is now false.
> The rewrite it called for has been done.

**What is true now.** `htgr_sim_v1` models a **pebble bed** at the published
HTR-10 operating point: 10 MWth, helium at 3.0 MPa, 250 → 700 degC at 4.3 kg/s,
27,000 fuel spheres in a 1.8 m × 1.97 m bed, downward flow, separate-vessel
once-through helical steam generator. Packed-bed pressure drop is real (KTA,
gold-gated), and every published constant is read from
`Htr10DesignPoint` rather than copied. The bed remains **one lumped control
volume** — a real friction correlation is not a resolved bed — and the
pebble-to-helium heat-transfer coefficient is still invented and measurably too
low. Nodalising the bed is tracked separately.

---

*Original text, 2026-08-06:*

**The existing `htgr_sim_v1` example is a prismatic-block HTGR, not a pebble
bed.** It says so at `crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/primary_loop.rs:3`.
There is no pebble bed anywhere in it: no packed-bed pressure drop, no bed
conductivity, no pebble conduction, no graphite properties.

**For HTR-10 the core model is a rewrite, not a retune.** The secondary loop,
the app shell and the widget layer are reusable almost as-is — which is a
substantial head start, but the core is new work.

## 2. Plant configuration

| Loop | Fluid | Purpose |
|---|---|---|
| Primary | Helium, **3.0 MPa** | Flows through a graphite-moderated pebble bed; multi-pass pebble recirculation |
| Secondary | Water / steam | Helical-coil once-through modular steam generator with helium on the shell side |

Passive safety case rests on the reflector, core barrel and reactor-cavity
cooling path. **Re-verified 2026-08-12: that route still has no model in this
workspace.** The *decay-heat source term* is no longer the gap it was — see the
decay-heat entry in the SCAFFOLD list — but nothing transports that heat out
through the reflector, barrel and cavity.

## 3. Capability audit

Originally audited 2026-08-06 against the workspace at commit `ebbde1b`.
**Fully re-verified 2026-08-12** against the working tree, with anything under
`examples/htgr_sim_v1/`, `src/components/` and `outram-park-fork-coolprop/`
read via `git show HEAD:` because other agents were editing those paths at the
time.

> **Caveat on `htgr_sim_v1` line numbers.** Every citation into
> `examples/htgr_sim_v1/` below is a **HEAD** line number, taken 2026-08-12
> while the example had uncommitted edits in the working tree (`panels.rs`,
> `schematic.rs`, `state.rs`, `physics/mod.rs`, `physics/secondary_loop.rs`
> were all dirty, and `physics/turbine_generator.rs` was untracked). The
> *symbols* named are what to search for; the *numbers* will drift as soon as
> those edits land. Same caveat for `src/components/` and
> `outram-park-fork-coolprop/`.

**Test counts, measured 2026-08-12 from `git show HEAD:` (`grep -c '#[test]'`
summed over each example's tracked files):** `htgr_sim_v1` **31** tests
(was recorded as 12), `fhr_sim_v2` **19** tests (was recorded as 3). **Whether
they pass was not re-run** — see the status note at the top.

> **Read the details, not just the verdict.** The single most common failure
> mode found in this re-audit was a claim that was *substantively correct*
> while its file path, line numbers and counts had all rotted. A citation
> pointing at the wrong line is worse than no citation, because it reads as
> authoritative. Every line number below was opened and confirmed on
> 2026-08-12.

### HAVE

| Capability | Where | Notes |
|---|---|---|
| **Helium equation of state and transport** | `crates/outram-park-fork-coolprop/src/fluids/helium.rs:14` (`pub static HELIUM: FluidEos`); `src/transport.rs:903` (`fn helium_viscosity`, Arp-McCarty-Friend, NIST TN-1334) and `:928` (`fn helium_conductivity`, Hands & Arp), dispatched from the enum arms at `:425` and `:655` | Full Helmholtz EOS plus real NIST-lineage viscosity and conductivity correlations. **Re-verified, details corrected 2026-08-12** — the old citation `helium.rs:13, transport.rs:710,732` pointed into a function *body*; `transport.rs` has grown to 61,668 bytes and the correlations moved |
| Helium consumed live by the existing sim | `examples/htgr_sim_v1/physics/primary_loop.rs:150` (`use outram_park_fork_coolprop::{state_pt, viscosity, Fluid}`) and `:663` (`fn helium_properties`) | Real EOS *and* real viscosity are now consumed, with a KTA 3102.1 correlation as the fallback when the flash fails — not a hardcoded constant. The heat-capacity-vs-ideal-gas-limit test is at `:737`. **Re-verified, details corrected 2026-08-12** — the old citation `:314` is now an unrelated flow-clamp constant (`MAX_HELIUM_FLOW_KG_PER_S`) |
| **Wakao packed-bed particle-to-fluid Nusselt** | `crates/tuas_boussinesq_solver/src/lib/heat_transfer_correlations/nusselt_number_correlations/input_structs.rs:160` (`pub struct WakaoData`), evaluated by `WakaoData::get` at `:190` | Reynolds and Nusselt on pebble diameter — directly the right form. **Re-verified, details corrected 2026-08-12** — `:152` is one line of the citation block, not the item; the full path was also elided in the original |
| Three-array porous-media component | `crates/tuas_boussinesq_solver/src/lib/pre_built_components/non_insul_porous_media_fluid/` | Fluid array, shell, interior solid matrix with real radial nodal conductances. The correct *structural* template for a pebble bed. **Re-verified 2026-08-12** — path unchanged |
| **Doubly heterogeneous Monte Carlo transport** | `crates/outram-mc-libs/src/pebble_beds/` (`delta_tracking.rs`, `keff_delta.rs`, `sphere_packing.rs`, `crp_packing.rs`, `references.rs`) | Woodcock delta tracking, k-eigenvalue power iteration over packed kernels, Random Sequential **Addition** sphere packing. **Re-verified, details corrected 2026-08-12** — the original said "adsorption"; the code says Addition, and `crp_packing.rs` has since been added to reach the ~0.55-0.62 packing fractions RSA's ~0.38 ceiling cannot |
| **TRISO fission-product diffusion and release** | `crates/boon-lay` — `lagrangian_decay_simulator/lagrangian_diffusion/first_passage/sphere_fpt.rs`, `triso_atops_fork/` | Four-layer TRISO CSG, Lagrangian Monte Carlo with walk-on-spheres first passage, temperature-dependent per-layer diffusion, closed-form Booth solution. Includes a port of INL's TRISO release code. **Re-verified 2026-08-12.** Note the release model is **unvalidated** — see [Defect worth its own bead](#defect-worth-its-own-bead) |
| Granular DEM with thermal contact | `crates/outram-park-fork-liggghts` — `contact.rs`, `rolling.rs`, `thermal.rs`, `thermal_radiation.rs`, `bonded.rs`, `mesh_wall.rs` | Hooke and Hertz-Mindlin contact, rolling resistance, contact conduction, grey-body radiation with near-field gas-gap conduction. **Re-verified 2026-08-12** |
| Point kinetics, Doppler, rod worth, xenon | `crates/teh-o-prke` — `zero_power_prke/six_group_precursor_prke/`, `delayed_neutron_layer`, `nordheim_fuchs`, `control_rod_feedback.rs:14`, `feedback_mechanisms/fission_product_poisons/` | Six-group PRKE, delayed neutron layer, closed-form prompt excursion, S-curve rod worth (Lamarsh's integral form $\rho(x) = \rho(H) \left[ x/H - \frac{1}{2\pi} \sin(2\pi x/H) \right]$, cited at `control_rod_feedback.rs:11` and implemented at `:28`), iodine/xenon dynamics. **Re-verified 2026-08-12** |
| **Working Rankine secondary cycle** | `examples/htgr_sim_v1/physics/secondary_loop.rs` | Real feedwater pump work, isentropic expansion, condenser energy balance, lagged feedwater control. **Seven** `#[test]`s with methodology and results documented. **Re-verified, details corrected 2026-08-12** — was "six tests"; count measured at HEAD, pass status not re-run |
| **Genuinely two-way primary/secondary coupling** | `examples/htgr_sim_v1/physics/mod.rs:268` (`let secondary_sink = self.secondary.saturation_temperature();`), with the rationale at `:52-56` and `:262-266` | Secondary saturation temperature is read before the primary step and used as the steam-generator cold-side pinch, so core inlet is a computed variable. Covered by a test. **Re-verified, details corrected 2026-08-12** — `:119` is now a `use` statement |
| Engine widget, animation and threading framework | `crates/outram-park-digital-twin-engine/src/` | Plus a working OPC-UA telemetry path (`src/opcua_core/`, `src/ciet_opcua/`, `src/bin/ciet_v2_opcua_client/`) if the twin ever needs one. **Re-verified 2026-08-12** |
| **NEW since the 2026-08-06 audit — cited HTR-10 design data and closures** | `crates/outram-park-digital-twin-engine/src/htr10/` — `design.rs`, `kta.rs`, `zbs.rs`, `neutronics.rs` | Four modules, not the two named in the gap-table corrections below. `design.rs` holds the published operating point; `kta.rs` the KTA 3102.3 packed-bed friction correlation (250 lines, 3 tests); `zbs.rs` the tabulated Zehner-Bauer-Schlunder bed conductivity (195 lines, 3 tests); `neutronics.rs` the IAEA core-physics benchmark specification and published reference eigenvalues. Module status is self-declared **NOT VALIDATED** (`htr10/mod.rs`) — the tests establish correct transcription and reproduction of published worked examples, not simulator validation. **Added 2026-08-12** |

### SCAFFOLD — do not count as working

- **Ergun is still unimplemented *in TUAS*.** The packed-bed variant is declared
  with its citation, and marked "not done yet", at
  `crates/tuas_boussinesq_solver/src/lib/array_fluid_collections/fluid_array_lateral_coupling/fluid_component_calculation/mod.rs:48-54`,
  with the match arm at `:149`. The gFHR pebble-bed components carry the comment
  "not putting in ergun equation yet" **eighteen times**, using pipe friction on
  a pebble-derived hydraulic diameter instead.

  **This is now a TUAS-specific gap, not a workspace-wide one** — see the
  correction below the gap table. Re-verified 2026-08-12 (the module path and
  the occurrence count in the previous wording were both stale).
- The TUAS porous-media component states its own gap: pressure-drop correlations
  are not properly implemented, so it behaves like a pipe. **Re-verified
  2026-08-12** — the comment is exact, at
  `crates/tuas_boussinesq_solver/src/lib/pre_built_components/non_insul_porous_media_fluid/mod.rs:60-61`.
- `fhr_sim_v2`'s pebble-bed thermal hydraulics is a **single lumped enthalpy
  control volume** for a UO2 pebble with externally supplied heat transfer
  coefficient and area, defaulting to a constant-heat-capacity heuristic. It is
  UO2, not graphite matrix, so it does not transfer to HTR-10. **Re-verified
  2026-08-12** — `examples/fhr_sim_v2/app/prke_backend/pebble_bed_thermal_hydraulics.rs`,
  142 lines, one state field (`current_fuel_specific_enthalpy`), and the
  heuristic is an explicit constant `cp = 340 J/(kg K)` at `:69` and `:77`.
  Unchanged in substance.
- **CFD-DEM coupling is an explicit no-physics stub** —
  `crates/outram-park-fork-liggghts/src/coupling.rs:25` states that every
  behavioural method returns not-implemented, with no drag law, no interpolation,
  no volume averaging and no fluid solve. **Re-verified 2026-08-12** — the line
  number is exact and `NotImplemented` appears 29 times in that file. The module
  documents this as "the *correct and intended* state of this phase, not a
  shortfall".
- **~~Decay heat is self-flagged as suspect.~~ CORRECTED 2026-08-12 — this is now
  false.** `crates/teh-o-prke/src/decay_heat.rs` was rewritten in commit
  `734a530759` ("HTR-10 foundations: 23-group decay heat, KTA/ZBS tests, kovan
  metadata fixes"). It is now a **23-group fit of the 1978 draft ANS Standard**
  (England *et al.*, 1978), transcribed from Tobias, "Decay heat", *Progress in
  Nuclear Energy*, Table 16, p. 78, with the source and its access tier
  documented in the module header. 683 lines, **7** `#[test]`s including
  `constant_irradiation_matches_published_integral_equation` and
  `shutdown_decay_heat_has_the_expected_magnitude_and_falls`. A grep for
  self-doubt wording (`not sure`, `unsure`, `suspect`, `doubt`, `wrong unit`)
  over the file returns **zero** hits — the comment the original audit cited no
  longer exists, and `:12` is now an unrelated module-scope doc line.

  **Still true:** it is **not wired into `htgr_sim_v1`** (`git grep -c` for
  `decay_heat|DecayHeat` under `examples/htgr_sim_v1/` at HEAD returns nothing).
  It *is* wired into `fhr_sim_v2`, at
  `examples/fhr_sim_v2/app/prke_backend/mod.rs:6`.

  **Not re-checked:** whether the model is numerically *right*. It now has a
  cited source and self-consistency tests, but no maintainer V&V sign-off, so
  under `VERIFICATION_AND_VALIDATION.md` it is verified-not-validated. The
  original "suspect" framing is withdrawn; a claim of trustworthiness is **not**
  substituted for it.
- `htgr_sim_v1`'s live steam pressure is hard-fixed, so there is no
  sliding-pressure or drum dynamics. **Re-verified 2026-08-12** — the value is
  the published 4.0 MPa read from `design()`, held fixed at
  `examples/htgr_sim_v1/physics/secondary_loop.rs:105-109`, and the module
  header states the limitation itself at `:60-62`.
- The `crates/tampines` **`src/components/` layer** returns not-implemented;
  only `Pipe::step` is real. **Re-verified, details corrected 2026-08-12** —
  measured: seven of the eight component modules (`condenser`, `cooling_tower`,
  `heat_exchanger`, `pump`, `steam_generator`, `turbine`, `valve`) each return
  `TampinesError::NotYetImplemented`; `pipe.rs` contains zero occurrences and
  its `step` is at `:111`.

  **The original wording — "the whole `crates/tampines` component layer" — is
  now misleading and has been narrowed.** The *crate* is no longer scaffold: it
  carries 93 `#[test]`s, and `src/pebble_bed/` alone is 5,656 lines with 34
  passing tests (see the MISSING table). The unimplemented set is specifically
  `src/components/`, whose own module doc explains why: those bodies need a real
  property-package flash, and the struct shapes were the deliverable of that
  pass.

### Defect worth its own bead

**A TRISO "verification" test does not verify anything.** **Re-verified
2026-08-12: the defect is real, still present, and is worse than recorded — it
is duplicated in two files, and the cited line number was wrong.**

The bead **`op-jyyp.10`** ("boon-lay DEFECT: TRISO release verification test
asserts against itself, not the reference", P2, bug, **Todo**) already exists,
created 2026-08-10. It is **not** closed, and it names only one of the two
files and the stale line number, so it should be updated rather than re-filed.

Both occurrences are in `crates/boon-lay/src/lagrangian_decay_simulator/lagrangian_diffusion/single_particle_simulator/`,
in the `test_cs_release_1200c_200h` test:

| File | `catch_unwind` at | Off-reference assertion at |
|---|---|---|
| `release_fraction_crp_6_case_1a_1b.rs` | `:56` | `:64` |
| `release_fraction_analytical_solution.rs` | `:192` | `:200` |

In each, the assertion against the published range — `fractional_release >= 0.453 && fractional_release <= 0.498`,
attributed to Hales, Jiang, Toptan & Gamble (2021), *J. Nucl. Mater.* **548**,
152840, Table 4 — is wrapped in `let _ = catch_unwind(|| { ... });`, so its
result is **discarded**. The next statement is
`approx::assert_relative_eq!(fractional_release, 0.53, max_relative = 0.01)`,
and **0.53 lies outside the 0.453-0.498 range it just declined to enforce**.
The test passes while the model disagrees with the reference it names in its own
filename.

**The companion 1600 degC test is not affected** — `test_cs_release_1600c_200h`
asserts its published range (`0.97` to `1.00`) directly, with no `catch_unwind`
(`release_fraction_crp_6_case_1a_1b.rs:97-101`).

The TRISO release model must be treated as **unvalidated** until this is fixed.
The original citation `:50` in this document was wrong: line 50 is a `println!`.

### MISSING

| Gap | Size | Notes |
|---|---|---|
| Ergun or KTA-form packed-bed pressure drop | ~~Small to write~~ **DONE (KTA)** | ~~Nothing exists~~ **`outram-park-digital-twin-engine/src/htr10/kta.rs` implements the KTA 3102.3 packed-bed friction correlation, tested, and it is WIRED into `examples/htgr_sim_v1`. Gold-gated against the Virtual Test Bed worked example: 3493.17 Pa/m vs 3493 Pa/m, +0.005%.** TUAS's own Ergun variant is still unimplemented — that gap is real but local to TUAS |
| **Pebble-bed effective radial conductivity** | ~~Medium~~ **EXISTS, not in a heat path** | ~~Literally zero code in the workspace~~ **`outram-park-digital-twin-engine/src/htr10/zbs.rs` implements Zehner-Bauer-Schluender, tested.** It is deliberately NOT in `htgr_sim_v1`'s heat path: one lumped bed control volume has no internal gradient for a conductivity to act on. Quantified 2026-08-12 — `k_eff(748.15 K) = 20.195 W/(m K)`, giving 11.74 kW of axial conduction, **0.117% of the 10 MW convective duty**. Negligible under forced flow; it becomes the entire heat path under LOFC, a regime that model cannot enter |
| **Graphite properties** — matrix and reflector grades, conductivity as a function of temperature and fast-neutron dose | ~~Medium~~ **EXISTS — CORRECTED 2026-08-12** | ~~The solid database holds only copper, stainless, fibreglass, aerogel, FeCrAl and a generic heating element. No graphite anywhere~~ **False.** `crates/tuas_boussinesq_solver/src/lib/boussinesq_thermophysical_properties/solid_database/nuclear_graphite.rs` — **887 lines, 6 tests** — implements **both** grades the row asked for: `SolidMaterial::NuclearGraphiteMatrixA3` (HTR-10 / HTR-PM pebble matrix, enum arm at `mod.rs:74`) and `SolidMaterial::NuclearGraphiteIG110` (reflector grade, `:82`), with a shared Butland & Maddison cp table, per-grade conductivity, and an **optional fast-neutron-fluence damage factor** (`nuclear_graphite_fluence_damage_factor`, `:196`). Densities 1730 and 1770 kg/m^3. Transcribed from the vendored CC-BY-4.0 Virtual Test Bed decks with per-function deck-file and line citations. **Caveat, stated by the module itself:** "None of these correlations has been checked against HTR-10 measurements by the maintainer" |
| Gas arm in the TUAS material enum | Small–Medium | `Material` is solid-or-liquid only, so the whole TUAS prebuilt component library cannot be used for a helium loop. **Re-verified 2026-08-12 — still true, unchanged.** `pub enum Material` at `crates/tuas_boussinesq_solver/src/lib/boussinesq_thermophysical_properties/mod.rs:14-19` has exactly two arms, `Solid(SolidMaterial)` and `Liquid(LiquidMaterial)`. Note the graphite row above lands in the **solid** arm, so it does not relieve this gap. **However, the gap has been routed around rather than closed:** `crates/tampines/src/gas_phase/` (**2,848 lines**) now provides a helium arm outside TUAS entirely — `properties.rs` (658 lines), `pipe.rs` (902), `circulator.rs` (445) and `kta_bed.rs` (734 lines, 8 tests). So a helium loop is buildable today; what remains blocked is specifically **TUAS's prebuilt component library** |
| Radial pebble conduction, fuel zone to surface | ~~Medium~~ **EXISTS — CORRECTED 2026-08-12** | ~~GeN-Foam's pebble routine was deliberately not ported; the app-builder crate says so explicitly~~ **The GeN-Foam half is still true** (`crates/outram-foam-appbuilder-lib/src/genfoam/thermal_hydraulics/structure/power_model.rs:31-34` and `structure/mod.rs:56` confirm the higher-fidelity radial pin/pebble models were not ported) — **but the gap is closed elsewhere.** `crates/tampines/src/pebble_bed/pebble.rs` — **1,237 lines, 8 tests** — implements two-zone spherical radial conduction: a fuelled zone with volumetric generation inside an unfuelled graphite shell, on the HTR-10 6.0 cm sphere / 5.0 cm fuelled zone geometry, with the double heterogeneity kept **explicit** rather than smeared. Its fuelled-zone conductivity is fed by `pebble_bed/triso.rs` (1,502 lines, 8 tests, five-region TRISO series resistance) |
| **Reflector, barrel and cavity-cooling decay-heat path** | Medium–Large | This is the HTR-10 passive safety case. No model. **Re-verified 2026-08-12 — still true, and now the largest gap on this slate.** Evidence: a case-insensitive `grep` over every crate's `src/` for `cavity cool`, `reactor cavity`, `RCCS` and `vessel cooling` returns **zero** hits. `core barrel` appears only as a *dimension* in unrelated components (`tuas .../gfhr_pipe_tests/components.rs:31`, `tampines/src/gas_phase/kta_bed.rs:176`) and `reflector` only as a **neutronics** node (`bedok`'s NEACRP axial reflector planes) — nothing transports heat out through the reflector, barrel and cavity |
| Multi-pass pebble flow and recirculation | Medium | DEM primitives exist but are unvalidated. **Re-verified 2026-08-12 — still true.** `crates/outram-park-fork-liggghts` has the contact/rolling/thermal primitives; no recirculation or multi-pass burnup-zone model exists |
| Helical-coil once-through SG, three-zone moving boundary | Medium | **Re-verified, details corrected 2026-08-12 — still true.** The LMTD and NTU algebra does exist, in `crates/outram-park-fork-dwsim-libs/src/heat_exchanger/` (`lmtd.rs`, `ntu_effectiveness.rs`, `f_correction.rs`). The zone tracking and helical correlations do not: `grep -i helical` over that crate returns **zero** hits. The only `Helical` in the workspace is a **geometry/visual archetype**, `SteamGeneratorKind::HelicalCoil` at `crates/outram-park-digital-twin-engine/src/components/steam_generator.rs:124` — a drawn 2.5 m / 11.3 m vessel, not a thermal model |
| SG inventory giving sliding steam pressure | Medium | The real remaining content of bead `op-wqk.9.3`. **Re-verified 2026-08-12 — still true**; `htgr_sim_v1`'s secondary loop header states the same at `secondary_loop.rs:20-25` |
| **Graphite/moderator temperature feedback as a separate channel** | ~~Small–Medium~~ **EXISTS — CORRECTED 2026-08-12** | ~~Central to HTR-10 loss-of-flow behaviour. Only lumped fuel feedback exists~~ **False.** `crates/tampines/src/pebble_bed/feedback.rs` — **995 lines, 6 tests** — carries the graphite moderator temperature as an independent state with its own thermal inertia and its own reactivity coefficient, deliberately *not* folded into Doppler, with the physical argument for the separation written out in the module header. Tests include `graphite_thermal_time_constant_is_long`, `thermal_time_constant_emerges_from_the_integrated_ode` and `the_lumped_balance_conserves_energy` |
| ~~Trustworthy decay heat~~ **WITHDRAWN — CORRECTED 2026-08-12** | — | ~~See the flagged defect above~~ The premise of this row was the "self-flagged as suspect" claim, which is false — see the SCAFFOLD list. `decay_heat.rs` is now a cited 23-group ANS-1978 fit with 7 tests. **This is not a claim that it is validated**: it is verified-not-validated, and it is still not wired into `htgr_sim_v1`. What remains is a *wiring* task, not a trust gap |
| Wall-friction or porous-drag source in the compressible pipe solver | Medium | **Re-verified, details corrected 2026-08-12 — still true.** The solver is the vendored `rhoPimpleFoam` under `crates/outram-park-fork-coolprop/src/openfoam_algorithms/`, not a module called "compressible". Its momentum equation (`rhoPimpleFoam/mod.rs:984-1019`) assembles only `ddt` + the deferred KNP dissipation + the pressure gradient; there is no wall-shear or Darcy source. `rhoPimpleFoam/lateral_coupling.rs:12` records a `DimensionlessDarcyLossCorrelations` port as future work |
| Nuclear data for a from-first-principles criticality calculation | Large | `reference-data/endf/` holds only a README. **Re-verified 2026-08-12 — still true**, `reference-data/endf/` contains exactly one file, `README.md`. (This is *not* contradicted by the Virtual Test Bed vendoring noted in section 4 — that supplies input decks and benchmark specifications, not evaluated nuclear data) |
| **Pebble-bed closures now exist as a coherent stack — recorded here so the gap table is not read as a to-do list** | — | **Added 2026-08-12.** `crates/tampines/src/pebble_bed/` is **5,656 lines with 34 tests, all passing** (`cargo test --release -p tampines --lib pebble_bed`, 34 passed / 0 failed, run 2026-08-12): `triso.rs` (level 1), `pebble.rs` (level 2), `cht.rs` (level 3, Wakao-Funazkri bed-to-helium), `zbs.rs` (effective bed conductivity), `feedback.rs` (graphite channel). **It is not consumed by any other crate** — no simulator is wired to it, so none of this is validated against HTR-10. Bead `op-jyyp.17`, which tracks exactly this work, is still open and appears stale. **There are now three KTA implementations in the workspace** — `dt-engine/src/htr10/kta.rs` (250 lines), `tampines/src/gas_phase/kta_bed.rs` (734 lines, 8 tests) — and **two** ZBS implementations. That duplication is worth a maintainer decision. **Also flagged:** `tampines/src/gas_phase/kta_bed.rs:13` claims Wakao particle-to-fluid heat transfer "is still unimplemented anywhere in the workspace", which is false — `tampines/src/pebble_bed/cht.rs` and TUAS's `WakaoData` both implement it. That is an in-code doc defect, reported not fixed |

### Bead accuracy — three of four children are stale

Bead states re-read 2026-08-12 with `bn show`. **None of the four has changed
since 2026-07-15**, so the "recorded state" column is still correct as written;
the "reality" column is updated below.

| Bead | Recorded state (confirmed 2026-08-12) | Reality |
|---|---|---|
| `op-wqk.9.1` helium TH are scaffold placeholders | Todo | **Now almost entirely stale — details corrected 2026-08-12.** Helium `c_p` and density are real EOS. **The viscosity claim is no longer true**: `primary_loop.rs:684` calls `outram_park_fork_coolprop::viscosity` and falls back to the KTA 3102.1 correlation `mu = 3.674e-7 T^0.7` (`:670`) only when the flash fails — it is not a hardcoded constant. **The friction claim is also stale**: the Haaland pipe friction was *removed* (`primary_loop.rs:12`) and the bed term is now the KTA packed-bed correlation (`:76-84`, `:149`). Still true: **one lumped node** |
| `op-wqk.9.2` wire kinetics to the delayed neutron layer | Done | **Correct. Re-verified 2026-08-12** |
| `op-wqk.9.3` secondary is scaffold | Todo | **Mostly stale.** The listed defects — simplified IHX duty, fixed secondary mass flow, no real turbine expansion or condenser balance — are all now implemented and tested. Only fixed steam pressure remains true. **Re-verified 2026-08-12** |
| `op-wqk.9.4` schematic omits the pipe widget | Todo | **Stale — the schematic uses it for every connector. Re-verified 2026-08-12:** `PipeVisual` appears **9** times in `examples/htgr_sim_v1/app/schematic.rs` at HEAD, and `:49` documents it as covering "every connector run and every elbow" |

**Two further bead candidates surfaced by this re-audit** (reported, not acted
on — closing or editing beads is the maintainer's call):

- **`op-jyyp.17`** — "tampines: the remaining three pebble-bed conduction scales
  (TRISO, pebble, CHT) plus the feedback channel", **Todo**. All four named
  deliverables exist and their 34 tests pass. **Close candidate.**
- **`op-jyyp.10`** — the TRISO defect bead. Real and still open, but it cites the
  wrong line (`:50`) and only one of the two affected files. **Update candidate**
  — see [Defect worth its own bead](#defect-worth-its-own-bead).

> **See also [vtb-findings.md](vtb-findings.md)** — the vendored NRIC/INL
> Virtual Test Bed carries material that closes several gaps recorded below.
> For HTR-10 specifically it supplies confirmed report identifiers and reference
> values that were deliberately left unasserted here.

> **Corrected 2026-08-12.** This document originally said the primary loop runs
> at "approx. 7 MPa". That was wrong — it was written before the sources were in
> hand. **All five HTR-10 references agree on 3.0 MPa**; see
> [htr10-plant-data.md](htr10-plant-data.md), which now carries the sourced
> plant and piping figures including the hot gas duct geometry and the full
> primary pressure-drop budget.

## 4. Open validation data

**Access tier: openly published, and unusually good for this reactor.**

> **No report identifiers, benchmark numbers, or measured values are asserted
> here.** They must be obtained from the actual documents.

| Source | Confidence | Relevance |
|---|---|---|
| **IAEA coordinated research programme benchmark on HTGR performance**, pairing HTR-10 with Japan's HTTR for initial testing | High | The canonical HTR-10 neutronics validation target. Cases include first criticality (critical loading height), temperature reactivity coefficient and control-rod worth. Widely reproduced in the Monte Carlo literature |
| **Follow-on IAEA programme** extending to steady-state operation and transients | High | Coupled neutronics and thermal-hydraulics code comparison |
| **HTR-10 safety demonstration tests** — loss of forced cooling without scram, and control-rod withdrawal without scram | High | The best available transient validation targets. Expect digitised plots rather than tabulated data |
| **The HTR-10 design description paper** by Wu, Lin and Zhong in *Nuclear Engineering and Design* | High on authors, title and journal; volume and pages unverified | Standard open source for core geometry, power, helium conditions and SG arrangement |
| **OECD/NEA PBMR-400 coupled benchmark** | High | Not HTR-10, but the most completely specified open **pebble-bed** benchmark, covering steady state plus depressurised and pressurised loss-of-cooling and rod withdrawal. The right *first* target before attempting HTR-10 |
| **IAEA programme on HTGR fuel technology** — TRISO fission-product release cases | High | `crates/boon-lay` already names one in a filename, though that test is the flagged self-referential one |
| **German AVR and THTR-300 operating data**, and packed-bed afterheat-removal experiments | Medium | Directly relevant for validating a pebble-bed effective conductivity model |
| HTR-10 as an evaluated criticality-handbook case | **Low — verify independently** | Would not rely on this |

> **The table above was NOT re-verified on 2026-08-12 and remains deliberately
> unchecked.** No report identifier, volume, page range or benchmark value in it
> has been confirmed against an actual document. It is retained exactly as
> written on 2026-08-06. Do not cite anything from it without obtaining the
> source; do not treat the confidence column as evidence.
>
> **Note the tension with `vtb-findings.md`.** The blockquote above section 4
> says that document "supplies confirmed report identifiers and reference values
> that were deliberately left unasserted here." **`vtb-findings.md` was not
> audited in this pass**, so whether those identifiers are genuinely confirmed
> is **not re-checked**. If a source is needed, go to `vtb-findings.md` and the
> vendored decks rather than to the table above — but verify there too.

**~~Nothing HTR-10-specific is currently in this repository. `reference-data/`
contains only a README.~~ CORRECTED 2026-08-12 — this is false.** The NRIC/INL
**Virtual Test Bed is now vendored** under `reference-data/virtual_test_bed/`,
including 14 HTGR case directories — among them **`htgr/htr10/steady/`** (an
HTR-10 case), plus `htgr/pbmr400/`, `htgr/htr-pm/`, `htgr/httr/`,
`htgr/generic-pbr/` and `htgr/generic-pbr-tutorial/`. These decks are the
cited source for the KTA and ZBS gold values and for the nuclear-graphite
correlations recorded above, and `docs/reactor-scoping/vtb-findings.md` catalogues
what they contain.

**Still true:** `reference-data/endf/` contains only a `README.md`. Input decks
and benchmark specifications are not evaluated nuclear data, and the
criticality-calculation gap in the MISSING table stands.

## 5. Recommended sequencing

1. **Target the PBMR-400 benchmark before HTR-10.** It is the most completely
   specified open pebble-bed case, and it exercises exactly the coupling that
   must be built. Reaching HTR-10's measured transients with an unvalidated bed
   model would be guessing.
2. ~~Build the bed closures first — pressure drop, effective conductivity,
   graphite properties — since every downstream result depends on them.~~
   **Done as of 2026-08-12; the sequencing advice is superseded.** All three
   exist and are tested. **The live task is the one this list did not
   anticipate: wire them into a solver.** Closures with no consumer cannot be
   validated, and `htgr_sim_v1` still runs one lumped bed control volume.
3. ~~Add the graphite feedback channel before attempting any loss-of-flow
   transient.~~ **Written, not wired** — `tampines/src/pebble_bed/feedback.rs`
   exists with 6 passing tests and no consumer. The advice stands in the form:
   do not attempt a loss-of-flow transient until that channel is *connected*.
4. The reflector and cavity-cooling path is what makes HTR-10 interesting. Budget
   for it rather than deferring it indefinitely. **Re-verified 2026-08-12 — still
   unbuilt, and now the largest single gap on this slate.**

## 6. Proposed work breakdown

Status column added 2026-08-12 from the re-verified audit above. **Six of the
fifteen items are done or superseded**, which is why this section should not be
read as a live to-do list without the column.

| Bead | Work | Status (2026-08-12) | Depends on |
|---|---|---|---|
| `htr10_sim_v1` | Parent; distinct from the prismatic `htgr_sim_v1` | **Superseded** — `htgr_sim_v1` was retargeted in place; no sibling exists. Tracked as `op-jyyp` | — |
| Refresh the stale `op-wqk.9.*` children | Bookkeeping against current reality | **Outstanding** — unchanged since 2026-07-15 | — |
| Graphite properties in the solid database | Matrix and reflector grades, conductivity versus temperature and dose | **DONE** — `nuclear_graphite.rs`, both grades, T- and fluence-dependent, 6 tests. Not maintainer-checked against HTR-10 measurements | — |
| Ergun / KTA packed-bed pressure drop | Implement and wire into the friction path | **DONE (KTA)** — implemented, wired, gold-gated at +0.005% | — |
| Pebble-bed effective radial conductivity | Solid, gas, contact and radiation contributions | **DONE, twice** — `dt-engine/src/htr10/zbs.rs` (195 lines, 3 tests) and `tampines/src/pebble_bed/zbs.rs` (1,112 lines, 7 tests). Neither is in a heat path | Graphite properties |
| Gas arm in the TUAS material enum | Unlocks the prebuilt component library for helium | **Outstanding** — `Material` still has exactly two arms, `Solid` and `Liquid` | — |
| Radial pebble conduction | Fuel zone to pebble surface | **DONE** — `tampines/src/pebble_bed/pebble.rs`, 1,237 lines, 8 tests. No consumer | Graphite properties |
| Reflector, barrel and cavity cooling path | The passive decay-heat route | **Outstanding** — no model found | Bed conductivity |
| Graphite temperature feedback channel | Separate from fuel feedback | **DONE** — `tampines/src/pebble_bed/feedback.rs`, 995 lines, 6 tests. No consumer | — |
| ~~Fix or replace decay heat~~ | ~~Currently self-flagged as suspect~~ | **Superseded** — the premise was false; `decay_heat.rs` is a cited 23-group ANS-1978 fit with 7 tests. The remaining task is **wiring it into `htgr_sim_v1`**, plus maintainer V&V | — |
| Fix the self-referential TRISO release test | Correctness defect in `crates/boon-lay` | **Outstanding** — bead `op-jyyp.10`, Todo. Present in **two** files | — |
| Helical-coil once-through SG | Three-zone moving boundary | **Outstanding** — only a drawn geometry archetype exists | — |
| SG inventory giving sliding steam pressure | Closes the real `op-wqk.9.3` gap | **Outstanding** | Helical SG |
| V&V against the PBMR-400 benchmark | Methodology and measured results per the workspace V&V rule | **Outstanding** — but the VTB `htgr/pbmr400/` deck is now vendored, so the input side is in hand | Bed closures, feedback channel |
| V&V against HTR-10 initial criticality and safety demonstration tests | Same | **Outstanding** | PBMR-400 first |

## 7. Open questions for the maintainer

1. **~~Separate simulator, or generalise the existing one?~~ ANSWERED — the
   question is moot as of 2026-08-12.** It was premised on `htgr_sim_v1` being
   prismatic; it was retargeted in place to the HTR-10 pebble-bed operating
   point, so no sibling example was created and the app shell was not
   duplicated. See [section 1](#1-framing-correction--read-this-first).
2. **Should the stale `op-wqk.9.*` beads be refreshed now?** Three of four
   describe defects that have since been fixed, and **none has changed state
   since 2026-07-15** (re-checked 2026-08-12). I have not touched them; closing
   or editing beads is your call.
3. **~~Does the self-referential TRISO test warrant a `crates/boon-lay` bead
   immediately?~~ ANSWERED — a bead exists.** `op-jyyp.10` was filed 2026-08-10
   and is still Todo. The open question is now narrower: it needs its line
   citation corrected and the **second, duplicate** occurrence added.
4. **New, 2026-08-12: should `crates/tampines/src/pebble_bed/` be wired into
   `htgr_sim_v1`?** 5,656 lines and 34 passing tests of pebble-bed closures now
   exist with **no consumer**. Until something calls them they cannot be
   validated, and the simulator keeps its one lumped bed control volume. The
   engine also has a second, independent ZBS implementation in
   `src/htr10/zbs.rs` — two ZBS models in one workspace is a duplication worth a
   decision.
