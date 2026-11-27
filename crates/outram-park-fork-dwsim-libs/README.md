# outram-park-fork-dwsim-libs

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


> **This is OUTRAM PARK's independent Rust translation of selected DWSIM
> algorithms.** It is not the official DWSIM software and is not affiliated
> with, endorsed by, or sanctioned by DWSIM Inc. or its maintainers. See
> [`TRADEMARKS.md`](./TRADEMARKS.md) for the full attribution and
> non-affiliation notice. Translated from
> [`DanWBR/dwsim`](https://github.com/DanWBR/dwsim), ~~`dwsim8`/`master` branch
> (confirm the current default branch when cloning) — no commit is pinned
> (no persistent local clone is currently maintained)~~ **CORRECTED
> 2026-11-27** — pinned at commit `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`
> (2026-07-17): the crate's `AGENTS.md` pins it, `upstream_source/README.md`
> records it as the commit at last sync, and every V&V record in
> `verification_and_validation/` was run against DWSIM 9.0.5.0 built from it
> (the branch name differs between `AGENTS.md`, `windows`, and
> `upstream_source/README.md`, `master`: *not re-checked*); see
> `upstream_source/README.md` for the full provenance record.

Pure-Rust port of DWSIM's chemical-process modelling kernels — thermal-
hydraulics and thermodynamics (flash algorithms, property packages/EOS,
equipment models).

**Name.** `outram-park-fork-dwsim-libs` is an OUTRAM PARK fork, named for its
upstream; the backronym is OUTRAM PARK's own, letter by letter: **O**pen-source
**U**nified **TRA**nsient **M**ulti-Physics **A**dvanced **R**eactor simulation
**K**it.

**The question it answers:** what does DWSIM's chemical-process thermodynamics
(equations of state, activity models, flash, columns, reactors, unit
operations) compute for a given feed and specification, written out in Rust
that can be read beside the VB.NET and C# it came from?

**What this crate makes studyable and challengeable.** A process-simulator
thermodynamics kernel, from the Peng-Robinson cubic through NRTL, UNIQUAC and
UNIFAC to the inside-out flash, the MESH column solvers and five reactor
models, each module citing the DWSIM `.vb` file it translates. Because upstream
DWSIM was built from source and run headless (`docs/upstream-harness/`, one C#
driver per model), each comparison can be re-run, and several have already
challenged both sides: the records list port defects fixed (e.g. #477, #481,
#482) and upstream defects found (e.g. #326, #478, #480, #485, #487 to #490).
It is a body of code to study and question, not a reference to trust.

## Knowledge in this crate

See the workspace [vision](../../VISION.md).

| Knowledge artifact | In this crate |
|---|---|
| Scientific literature | Upstream: DWSIM (D. Medeiros, GPL-3.0), commit `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`, provenance in `upstream_source/README.md`. Per-model literature in module doc comments (e.g. Petalas and Aziz (2000), J. Can. Pet. Technol. 39(6), in `src/pipe/petalas_aziz.rs`). `docs/chemistry-model-survey.md` surveys DWSIM's models and their sources. No literature in `crates/kovan-literature/CATALOGUE.md` is tied to this crate. |
| Mathematical equations | Module docs of `src/thermo/` (cubic EOS and alpha functions, activity models, flash formulations, Michelsen stability, Gibbs minimisation), `src/columns/` (MESH equations, Wang-Henke, Naphtali-Sandholm, shortcut FUG), `src/reactors/` and `src/reactions.rs` (Arrhenius power law, `K_eq(T)`, Langmuir-Hinshelwood), `src/pipe/` (Darcy-Weisbach, Beggs and Brill, Lockhart-Martinelli). Scope: `docs/port-scope.md`, `docs/unifac-port-scope.md`. |
| Source code | `src/thermo/` (EOS, activity, flash family, electrolytes, sour water, transport), `src/columns/`, `src/reactors/`, `src/reactions.rs`, `src/flowsheet/` and `src/flowsheet_solver/`, `src/dynamics/`, `src/petroleum/`, unit operations (`pipe`, `valve`, `heat_exchanger`, `compressor`, `expander`, `pump`, `heater`, `cooler`, `mixer`, `splitter`, `separator`), `src/clean_energies/`, `src/interpolation/`. |
| Interactive simulations | CLI examples only: `examples/pt_flash.rs`, `examples/crude_column_from_prelude.rs`. |
| Visualisations | None yet. |
| Verification and validation evidence | Code-to-code against upstream DWSIM 9.0.5.0 built from `1abf72d1`, not validation against experiment. `verification_and_validation/reactors_vs_upstream_dwsim.md` (2026-10-02): solver agreement conversion <= 3.7e-16, equilibrium <= 3.9e-12, Gibbs <= 8.5e-6 (GibbsMin), PFR 1.4e-10 / 2.6e-9 / 2.0e-10, CSTR 2.4e-13. `verification_and_validation/sour_water_vs_upstream_dwsim.md` (2026-10-02). `verification_and_validation/upstream_differential_gh1356/` (2026-11-13): NRTL 964 of 966 gammas bit-identical after fix, UNIQUAC 966 of 966, petroleum correlations 1280 of 1328 bit-identical (worst 4.2e-15), FUG shortcut 110 of 112. `AGENTS.md` (2026-09-13): Peng-Robinson `Z` equal to upstream `Z_PR` to 6 significant figures at four points. Tests: `tests/upstream_*_parity.rs`. `docs/upstream-port-coverage.md`: 0 rows `PORTED + VALIDATED`. |
| Human reviews and corrections | 0 functions carry a kovan human-review stamp. No entry in `docs/human-corrections-to-ai-work.md`. Struck-through corrections in `AGENTS.md` (the Peneloux volume-translation trace, the clone path) and in `docs/upstream-port-coverage.md` (row R6). |

**Status:** declared mature by the maintainer on 2026-09-13 (`AGENTS.md`). The
reactors, sour water, activity models, petroleum correlations, shortcut column
and the Peng-Robinson EOS have machine-run code-to-code evidence against
upstream DWSIM; none of it is human-reviewed, nothing is validated against
experimental data, and the flash and column layers' agreement with upstream
`CalcProp` is open (about 2 %, the Peneloux shift this port does not apply).

**Status: equipment-model correlations + a broad thermodynamics kernel
(EOS, activity, flash, electrolyte, Gibbs) + reactions/reactors landed.**
**CORRECTED 2026-11-27** — also present, verified in `src/lib.rs`: `columns`
(distillation and absorption, rigorous and shortcut), `flowsheet` and
`flowsheet_solver` (including the `.dwxml` / `.dwxmz` import), `dynamics`,
`petroleum`, `clean_energies` and `interpolation`; `thermo` also holds
`black_oil`, `chao_seader_grayson`, `flash_immiscible` and `seawater`, not
listed below.

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
per the crate `AGENTS.md`.

Everything here is **verified, not benchmark-validated** — ~~the tests check the
ports against analytical identities and hand-computed / published
pure-component reference points~~ **CORRECTED 2026-11-27** — the tests check the
ports against analytical identities, published pure-component reference
points and, since 2026-09-25, upstream DWSIM itself built from the pinned
commit (`tests/upstream_*_parity.rs`, `verification_and_validation/`) — not against experimental VLE/property
datasets. `tampines` is the intended downstream consumer.

See `docs/port-scope.md` and `docs/chemistry-model-survey.md` for the full
porting scope and per-model status, and the crate's GitHub issues for the
current backlog (the retired tracker's epic was `op-qo2`). Remaining deferred items include: Petalas-Aziz, full Tinker
shell-and-tube rating, the flash-coupled pressure update for the transient pipe
network, the Mathias-Copeman / Twu α-variants, the advanced-EOS tier
(PC-SAFT / GERG-2008), and the LIQUAC full-package glue.

## `no_std` + `alloc` (gh:#1591)

The default `std` feature can be turned off: the library is then `no_std` +
`alloc` and builds for bare metal
(`cargo check --release -p outram-park-fork-dwsim-libs --lib --no-default-features --target x86_64-unknown-none`,
gated by `scripts/check-nostd.sh`). Thermodynamics, the unit operations, the
flowsheet and its solver, and the dynamics engine build without it. What
`std` adds:

- `flowsheet::import`, the `.dwxml` / `.dwxmz` reader (files and `zip`).
- the wall clock (`crate::time::Instant`). Without it `elapsed()` is always
  zero, so a wall-clock timeout never fires and the iteration limits alone
  bound a solve.
- printing.

Maps come from `crate::hash`: std's `HashMap` with `std`, so iteration order
and every number are exactly as before, and `hashbrown` without it. With
`std` on (the default) nothing changes.

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

~~GPL-3.0-only~~ **AGPL-3.0-only** combined work (relicensed 2026-10-20, gh:#973; ported files keep GPL-3.0 and `LICENSE.GPL-3.0` ships beside `LICENSE`; see `docs/relicensing-agpl-2026-10-20.md`); the ported files match DWSIM's own
upstream license (GPL-3.0) directly. See
`TRADEMARKS.md` for the full non-affiliation notice.

## Copyright

Copyright (C) 2026 Ong Kay Chen Theodore, Professor Per F. Peterson,
University of California, Berkeley Thermal Hydraulics Lab,
Singapore Nuclear Research and Safety Institute (SNRSI),
National University of Singapore (NUS), Repository Contributors.
