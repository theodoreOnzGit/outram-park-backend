# outram-park-fork-moltres

**Name.** `outram-park-fork-moltres` is an OUTRAM PARK fork, named for its
upstream, Moltres; the backronym is OUTRAM PARK's own, letter by letter:
**O**pen-source **U**nified **TRA**nsient **M**ulti-Physics **A**dvanced
**R**eactor simulation **K**it.

**The question it answers:** how much reactivity does a molten-salt reactor
lose when its fuel circulates and carries delayed-neutron precursors out of
the core, and how do flux, precursors and salt temperature couple?

MSR neutronics + thermal-hydraulics on the outram-foam finite-volume layer — physics formulation from the LGPL-2.1 [Moltres](https://github.com/arfc/moltres) code, reimplemented on `outram-foam-basic-lib` rather than MOOSE/PETSc.

Circulating-fuel molten-salt reactor multiphysics: multigroup neutron diffusion, **delayed-neutron precursor drift** (the defining MSRE effect — precursors advected out of the core by the flowing fuel salt), and salt thermal-hydraulics, coupled on the OUTRAM PARK finite-volume mesh. Moltres is MOOSE/finite-element; this is an independent finite-volume reimplementation of the same validated formulation (no MOOSE/PETSc/MPI, per the workspace no-FFI rule).

**What this crate makes studyable and challengeable.** The precursor
advection-decay equation and the multigroup diffusion eigenproblem as a few
small finite-volume modules, readable beside the Moltres kernels whose
formulation they follow (each module's header names the kernels), with
source terms (`terms`) whose floating-point association follows Moltres so the
comparison is a bit-identity check, not a tolerance. The MSRE N-4 record works
the circulating-fuel reactivity loss end to end from ORNL-TM-1626, B. E.
Prince (1966), digitised figure, prediction posted before the run, and the
disagreement with the measurement reported. It is a reimplementation to study
and challenge, not a validated MSR code.

> **⚠️ Untrusted AI-assisted draft — no human V&V.** First-pass physics
> implemented and machine-verified ~~against analytic/limiting cases only~~
> **CORRECTED 2026-11-27** — against analytic and limiting cases, against
> Moltres `3dd2ce7` (kernel and gold-file level, gh:#1400, gh:#1538), and, for
> the precursor drift, against the MSRE N-4 measurement in ORNL-TM-1626
> (gh:#1130); see the sections below — under
> the MSRE digital-twin epic (`op-6w0`). Research, education and V&V only
> (`RESPONSIBLE_USE.md`). Independent OUTRAM PARK fork; not
> affiliated with the upstream project. Not for nuclear facility operation,
> reactor control, safety-critical, or licensing decisions.

## Knowledge in this crate

See the workspace [vision](../../VISION.md).

| Knowledge artifact | In this crate |
|---|---|
| Scientific literature | Upstream: Moltres (arfc/moltres, UIUC ARFC group, LGPL-2.1), commit `3dd2ce7`, provenance in the `src/lib.rs` header and `NOTICE`. ORNL-TM-1626, B. E. Prince (1966), for the MSRE N-4 case (`verification_and_validation/msre_n4_precursor_drift/README.md`). G. R. Keepin, *Physics of Nuclear Kinetics*, for the U-235 six-family delayed data (`src/materials.rs`). In `crates/kovan-literature/CATALOGUE.md`: `robertson1965msre` (ORNL-TM-728, MSRE design), listed for this crate's MSRE group. |
| Mathematical equations | Multigroup neutron diffusion k-eigenvalue (`src/diffusion.rs`); delayed-neutron precursor advection-decay-diffusion with six families, `dC_i/dt + div(u C_i) - div(D_C grad C_i) = beta_i/k S_f - lambda_i C_i` (`src/precursors.rs`); coupled circulating-fuel eigenvalue (`src/circulating.rs`); slug-flow salt energy balance with a heat-exchanger sink (`src/thermal.rs`); MOOSE linear, spline, monotone-cubic and bicubic interpolation (`src/moose_interpolation.rs`). The N-4 line model and its exact solution: `verification_and_validation/msre_n4_precursor_drift/README.md`. |
| Source code | `src/materials.rs`, `src/ring_mesh.rs`, `src/diffusion.rs`, `src/precursors.rs`, `src/circulating.rs`, `src/thermal.rs`, `src/terms.rs`, `src/tables.rs`, `src/tables_multivar.rs`, `src/moose_interpolation.rs`, `src/error.rs`. |
| Interactive simulations | None yet. |
| Visualisations | No result plots. `verification_and_validation/msre_n4_precursor_drift/images/tm1626_fig4_250dpi.png` is the rendered TM-1626 figure the inputs were digitised from. |
| Verification and validation evidence | Analytic checks (2026-08-04, table below). `verification_and_validation/moltres_differential_gh1400/RESULTS.md` (2026-11-13): interpolation and source-term kernels 100 % bit-identical to a C++ transcription of Moltres; `k_inf` within 6e-14 relative of Moltres' gold `k_eff`. `verification_and_validation/msre_n4_precursor_drift/README.md` (2026-11-09 and 2026-11-10): moltres drift agrees with the exact line-model solution to 4e-8; loss 0.2293 % on Prince's shapes, 0.2191 % on the N-1 Monte Carlo shapes, 0.2101 % with fast-flux importance, against the measured 0.212 +- 0.004 % δk/k. Tests in `tests/`. |
| Human reviews and corrections | 0 functions carry a kovan human-review stamp. No entry in `docs/human-corrections-to-ai-work.md`. The N-4 record strikes through its own "largest untested assumption" once the importance proxy was tested (2026-11-10). |

**Status:** the diffusion, precursor and interpolation kernels have machine-run
evidence against analytic solutions and Moltres, and the precursor drift has
been compared with one MSRE measurement through a line model; none of it is
human-reviewed, the full `CirculatingFuelSolver` has not been compared with
MSRE data (no published MSRE axial diffusion constants were found), and the
salt velocity is prescribed, not solved.

## `no_std` + `alloc` (gh:#1597)

The default `std` feature can be turned off: the library is then `no_std` +
`alloc` and builds for bare metal
(`cargo check --release -p outram-park-fork-moltres --no-default-features --target x86_64-unknown-none`,
gated by `scripts/check-nostd.sh`). `std` carries the table loaders that read files (`tables::...::load`, `tables_multivar::...::load`) and the `MOLTRES_DEBUG_OUTER` trace; JSON table parsing stays (`serde_json` with `alloc`, the root's `serde_json-nostd` key). With `std` on (the default)
nothing changes.

## What is implemented (first pass, 2026-08-04)

**CORRECTED 2026-11-27** — the list below is the 2026-08-04 first pass;
`src/lib.rs` also declares `terms` (Moltres-ordered source terms, gh:#1400),
`tables` and `tables_multivar` (Moltres property tables, JSON and
multivariate), `moose_interpolation` (gh:#1538) and `error`.

- **`materials`** — SI multigroup cross-section records (`MsrMaterial`,
  `DelayedFamily` with the Keepin U-235 6-family set) materialised to per-cell
  fields (`XsFields`); reduced linear temperature feedback on the removal
  cross section.
- **`ring_mesh`** — a closed 1-D loop `FvMesh` (`RingMesh`): periodic topology
  built as a ring of internal faces (no cyclic boundary machinery), cells laid
  on a circle so every adjacent-cell distance is uniform, plus a prescribed
  slug-flow face flux.
- **`diffusion`** — static-fuel multigroup diffusion k-eigenvalue
  (`StaticDiffusion`): power iteration over `fvm::laplacian + fvm::sp` group
  systems with warm-started CG.
- **`precursors`** — delayed-neutron precursor advection–decay drift
  (`PrecursorDrift`): steady solve and backward-Euler transient step of
  `dC_i/dt + div(u C_i) - div(D_C grad C_i) = beta_i/k S_f - lambda_i C_i`,
  assembled with upwind `fvm::div`.
- **`circulating`** — the coupled flux + drifting-precursor eigenvalue on the
  closed loop (`CirculatingFuelSolver`): reproduces the static solver exactly
  at `u = 0` and the MSRE circulating-fuel reactivity loss at `u > 0`.
- **`thermal`** — reduced slug-flow salt temperature with heat-exchanger sink
  (`SaltThermalModel`) and the Picard-coupled power/temperature-feedback loop
  (`CoupledMsrSolver`).

Everything is SI (metres); convert cm-based reactor-physics tables on input.
The salt velocity is **prescribed** (rigid loop circulation) — no CFD in this
crate by design; that is the `outram-foam-appbuilder-lib`/GeN-Foam path.

## Verification snapshot (measured 2026-08-04, release build)

Automated verification against analytic/limiting references — methodology and
numbers live in each test's doc comment (`src/*.rs`):

| Check | Result |
|---|---|
| 1-group bare-slab k vs analytic | rel. err `6.3e-6` |
| 2-group bare-slab k vs analytic | rel. err `9.5e-7` |
| Zero-flow precursor equilibrium | rel. err `2.3e-16` |
| Closed-loop precursor production = decay | imbalance `<= 8.6e-11` |
| Circulating solver at `u = 0` vs static solver | `dk = 2.2e-16` |
| **Circulation reactivity loss vs loop speed** | 151 / 216 / 287 / 348 / 388 pcm at 0.15 / 0.3 / 0.6 / 1.2 / 2.4 m/s — monotone, `< beta = 650 pcm`, MSRE-order at nominal flow |
| Closed-loop energy balance (HX removal = power) | imbalance `1.1e-8`; slug-flow heat-up matches analytic to 0.03 % |
| Temperature-feedback sign (k vs power) | monotone negative, ~170 pcm/MW at 0.5/4/8 MW |

~~This is **verification only** (correct implementation of the equations), not
validation against MSRE benchmark data.~~ **CORRECTED 2026-11-27** — true of
the table above. Since 2026-11-09 the precursor drift has also been compared
with the MSRE N-4 measurement (ORNL-TM-1626) through Prince's line model:
`verification_and_validation/msre_n4_precursor_drift/README.md`, pinned by
`tests/msre_n4_precursor_drift_tm1626.rs`. That is a validation of the line
model built on `PrecursorDrift`, not of the full coupled solver.

### Differential against Moltres 3dd2ce7 (gh:#1400, added 2026-11-13)

Kernel level (C++ transcription of the Moltres kernels + MOOSE
`LinearInterpolation`, bits compared) and solution level (values from
Moltres' committed gold files; MOOSE+Moltres was **not** rebuilt). Full
record: `verification_and_validation/moltres_differential_gh1400/RESULTS.md`.
Headline: table interpolation, `InScatter`, `DelayedNeutronSource`,
`PrecursorDecay`, prompt-fission term 100 % bit-identical; `k_inf` from the
Moltres property tables within `6e-14` relative of Moltres' `k_eff`
(`gmm_none`, `gmm_linear`); Marshak/Mark/Milne vacuum right-edge values
within `5e-14` absolute of the FE gold values.

Round 2 (gh:#1538): MOOSE `SplineInterpolation`, `MonotoneCubicInterpolation`
and `BicubicSplineInterpolation` (`moose_interpolation`), the least-squares
branch and bicubic tables (`tables_multivar`) and `MoltresJsonMaterial`
(`MoltresTables::from_json`) are translated. Spline, monotone-cubic, bicubic and
least-squares kernels are 100 % bit-identical to the C++ transcription; the
spline and monotone-cubic `k_inf` match the `gmm_*` and `mjm_*` gold `k_eff`
within `3.1e-14` relative. Bicubic and least-squares have no Moltres test input,
so they are kernel-level only. The `peak_power_density` controller is not
translated.

## Run the tests

```bash
cargo test -p outram-park-fork-moltres --lib --tests --release
```

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.
