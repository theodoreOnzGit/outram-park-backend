# outram-park-fork-dwsim-libs

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


> **This is OUTRAM PARK's independent Rust translation of selected DWSIM
> algorithms.** It is not the official DWSIM software and is not affiliated
> with, endorsed by, or sanctioned by DWSIM Inc. or its maintainers. See
> [`TRADEMARKS.md`](./TRADEMARKS.md) for the full attribution and
> non-affiliation notice. Translated from
> [`DanWBR/dwsim`](https://github.com/DanWBR/dwsim), `dwsim8`/`master` branch
> (confirm the current default branch when cloning) — no commit is pinned
> (no persistent local clone is currently maintained); see
> `upstream_source/README.md` for the full provenance record.

Pure-Rust port of DWSIM's chemical-process modelling kernels — thermal-
hydraulics and thermodynamics (flash algorithms, property packages/EOS,
equipment models).

**Status: equipment-model correlations + a broad thermodynamics kernel
(EOS, activity, flash, electrolyte, Gibbs) + reactions/reactors landed.**

*Equipment / unit-operation models* (`uom`-typed public APIs): `pipe`
(Darcy-Weisbach + Beggs & Brill + Lockhart-Martinelli two-phase pressure
drop), `valve` (IEC 60534 liquid/gas/two-phase Kv sizing), `heat_exchanger`
(LMTD, epsilon-NTU effectiveness, Bowman/Underwood multi-pass F-correction),
`expander` and `compressor` (isentropic + Schultz polytropic-efficiency
turbomachinery), `pump` (direct calculation modes + NPSH), `heater` / `cooler`
(enthalpy-driven duty), `mixer` (adiabatic mass/energy balance), `splitter`
(mass-balance stream tee), `separator` (two-phase flash drum — the first
equipment model that invokes the flash kernel directly), and the `reactors`
tier (conversion / equilibrium / CSTR / PFR / Gibbs-minimisation) built on the
`reactions` model (Arrhenius power-law, K_eq(T), Langmuir-Hinshelwood).

*Thermodynamics kernel* (`thermo`, ported from `DWSIM.Thermodynamics`): the
pure-compound `component` data model; the cubic EOS tier — Peng-Robinson / SRK
`cubic_eos`, PRSV + Peneloux `eos_variants`, PR78 `pr1978`, full PRSV2
`prsv2_full`, Lee-Kesler-Plöcker `lkp`, and the PR + Lee-Kesler caloric hybrid
`pr_lee_kesler`; the activity tier — `activity` (NRTL / UNIQUAC / ideal),
`unifac`, modified UNIFAC Dortmund `unifac_dortmund`, `unifac_lle`, and the
`electrolyte` aqueous-ionic tier; `ideal_props` ideal-gas enthalpy/entropy and
`transport` (viscosity / conductivity / surface tension). The flash family
spans isothermal-isobaric VLE (`flash` nested-loops, `flash_insideout`
Boston-Britt), three-phase VLLE (`flash_vlle`, `flash_insideout_3p`), LLE
(`flash_lle`), solid equilibria (`flash_sle`, `flash_svlle`), the
single-component shortcut (`flash_single_comp`), Gibbs-minimisation speciation
(`gibbs`, `gibbs_multiphase`), electrolyte SVLE (`electrolyte_svle`,
`sour_water`), the `energy_flash` (PH) driver, `saturation` (bubble/dew),
`stability` (Michelsen tangent-plane), and the `property_package` PT driver.
Enum dispatch, no `dyn`; documented raw-`f64` SI in the inner EOS/flash loops
per the crate `CLAUDE.md`.

Everything here is **verified, not benchmark-validated** — the tests check the
ports against analytical identities and hand-computed / published
pure-component reference points, not against experimental VLE/property
datasets. `tampines` is the intended downstream consumer.

See `docs/port-scope.md` and `docs/chemistry-model-survey.md` for the full
porting scope and per-model status, and `bd show op-qo2` for the current
backlog. Remaining deferred items include: Petalas-Aziz, full Tinker
shell-and-tube rating, the flash-coupled pressure update for the transient pipe
network, the Mathias-Copeman / Twu α-variants, the advanced-EOS tier
(PC-SAFT / GERG-2008), and the LIQUAC full-package glue.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.

## License

GPL-3.0-only (see the workspace root `LICENSE`), matching DWSIM's own
upstream license directly — no relicensing step is needed. See
`TRADEMARKS.md` for the full non-affiliation notice.

## Copyright

Copyright (C) 2026 Ong Kay Chen Theodore, Professor Per F. Peterson,
University of California, Berkeley Thermal Hydraulics Lab,
Singapore Nuclear Research and Safety Institute (SNRSI),
National University of Singapore (NUS), Repository Contributors.
