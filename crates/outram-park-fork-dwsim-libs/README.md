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

**Status: equipment-model correlations + a core thermodynamics kernel landed.**

*Equipment / unit-operation models* (`uom`-typed public APIs): `pipe`
(Darcy-Weisbach + Beggs & Brill + Lockhart-Martinelli two-phase pressure
drop), `valve` (IEC 60534 liquid/gas/two-phase Kv sizing), `heat_exchanger`
(LMTD, epsilon-NTU effectiveness, Bowman/Underwood multi-pass F-correction),
`expander` and `compressor` (isentropic + Schultz polytropic-efficiency
turbomachinery), `pump` (direct calculation modes + NPSH), `heater` / `cooler`
(enthalpy-driven duty), `mixer` (adiabatic mass/energy balance), `splitter`
(mass-balance stream tee), and `separator` (two-phase flash drum — the first
equipment model that invokes the flash kernel directly).

*Thermodynamics kernel* (`thermo`, ported from `DWSIM.Thermodynamics`): the
pure-compound `component` data model, Peng-Robinson / SRK `cubic_eos` with the
PRSV + Peneloux `eos_variants` refinements, `activity` (NRTL / UNIQUAC / ideal)
and `unifac` liquid-phase activity coefficients, `ideal_props` ideal-gas
enthalpy/entropy, the isothermal-isobaric `flash` and `property_package` PT
driver, the `energy_flash` (PH) driver, `saturation` (bubble/dew), `stability`
(Michelsen tangent-plane), and `transport` (viscosity / conductivity / surface
tension). Enum dispatch, no `dyn`; documented raw-`f64` SI in the inner
EOS/flash loops per the crate `CLAUDE.md`.

Everything here is **verified, not benchmark-validated** — the tests check the
ports against analytical identities and hand-computed / published
pure-component reference points, not against experimental VLE/property
datasets. `tampines` is the intended downstream consumer.

See `docs/port-scope.md` for the full prioritised porting scope and
`bd show op-qo2` for the current backlog status (deferred items include:
Petalas-Aziz, full Tinker shell-and-tube rating, flash-coupled pressure update
for the transient pipe network, the PRSV2/Mathias-Copeman/Twu α-variants and
LKP, three-phase / electrolyte / solid equilibria, and DWSIM's reactor tier).

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
