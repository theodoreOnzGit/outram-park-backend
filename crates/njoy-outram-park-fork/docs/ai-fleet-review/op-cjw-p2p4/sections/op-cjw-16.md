# op-cjw.16 — LEAPR MF=7 writer (`endout`) + `coldh` orchestrator + coherent-elastic (`coher`)

**Bead:** op-cjw.16 (P4)
**Scope touched:** `src/leapr/` only (plus this manifest). No edits to `src/lib.rs`,
`src/modules.rs`, `src/thermr/`, or any other dir.
**Upstream mirrored:** NJOY2016 `src/leapr.f90`, commit `ac5adf5f33d893e42f2eed7fb286b0d51c7580da`.

## Files changed

| File | Change | Lines |
|---|---|---|
| `src/leapr/coher.rs` | **new** — coherent-elastic Bragg lattice sum | 404 |
| `src/leapr/endout.rs` | **new** — ENDF-6 MF=7 tape writer | 520 |
| `src/leapr/coldh.rs` | replaced `NotPorted` stub with the ported `add_cold_hydrogen` orchestrator; 2 new tests | 607 |
| `src/leapr/discrete.rs` | exposed `bfill`/`exts`/`sint` as `pub(crate)` (were private) so `coldh` can reuse them | (edit only) |
| `src/leapr/mod.rs` | module map + "ported vs not ported" doc updated; re-exports `coher`, `endout`, `add_cold_hydrogen` | (edit only) |

All new files carry the GPLv3 + NJOY2016 provenance header (commit `ac5adf5`).
Every new file is < 1000 lines.

## Provenance map (Fortran routine → Rust fn)

| Fortran (`leapr.f90` lines) | Rust |
|---|---|
| `coher` (2489–2814) | `coher::coher` |
| `formf` (2924–2970) | `coher::formf` |
| `tausq` (2792–2797) | `coher::tausq` |
| `taufcc` (2799–2805) | `coher::taufcc` |
| `taubcc` (2807–2812) | `coher::taubcc` |
| `endout` MT=2 coherent (3192–3289) | `endout::build_coherent_elastic` |
| `endout` MT=2 incoherent (3158–3190) | `endout::build_incoherent_elastic` |
| `endout` MT=4 inelastic (3291–3618) | `endout::build_inelastic` + `stored_s`/`output_beta` |
| `endout` section assembly (3052–3620) | `endout::endout` |
| `coldh` (1936–2183) | `coldh::add_cold_hydrogen` |
| `bfill`/`exts`/`sint` (1798–1934) | reused from `discrete.rs` (now `pub(crate)`) |
| `bt`/`sumh`/`terpk` (2185–2466) | reused from `coldh.rs` (previously ported) |

## DONE vs PARTIAL / stub

### DONE (ported + tested)
- **`coher` + `formf` + `tausq`/`taufcc`/`taubcc`** — full lattice sum for all six
  built-in moderators (graphite, Be, BeO, Al, Pb, Fe). Debye-Waller factor is
  correctly inert here (`wint = 0` in the Fortran, 2593), matching NJOY.
- **`endout` MF=7 writer** — MT=2 coherent (`LTHR=1`, cumulative `S(E)` with the
  `jmax` 1/E thinning), MT=2 incoherent (`LTHR=2`, `W'(T)`), and MT=4 incoherent
  inelastic (B-constants LIST, TAB2 over beta, per-`(beta,temp)` `S(alpha)`
  TAB1/LIST, effective-temperature TAB1). `sigfig` 7-figure rounding and `smin`
  flooring ported via `crate::mixr::mix::sigfig`. All four `isym` branches
  (0/1/2/3) transcribed in `stored_s`/`output_beta`.
- **`coldh` orchestrator** — `add_cold_hydrogen` for ortho/para H2 and D2
  (laws 2–5). Faithful transcription of the `j`/`j'` rotational loops, spin
  factors, `sint` convolution, and the trapezoidal normalization check.

### PARTIAL / intentionally omitted (with Fortran line ranges)
- **MF=1/MT=451** descriptive header + Hollerith comment cards (`endout`
  3052–3156). Not written: no `S(alpha,beta)` physics, and the crate's `[f64; 6]`
  row model cannot store Hollerith text (same limitation MIXR documents). MF=7 is
  complete without it; the THERMR reader only consumes MF=7.
- **Mixed-moderator merge** (`nss != 0`, `endout` 3017–3030) and the secondary
  `ssp` scratch-tape plumbing — **not** ported. Single principal scatterer only.

### STILL `NotPorted` / absent
- **`copys`** (2468–2487) — scratch-tape plumbing for the mixed-moderator path;
  unneeded for the single-scatterer in-memory pipeline.
- **`skold`** (2816–2922) — Sköld pair-correlation correction; still absent.
- **NJOY `leapr` card-reading driver / `run()`** — still returns
  `NjoyError::NotPorted`; callers build a `LeaprInput` and call the module
  functions directly, then `endout::endout` for the tape.

## LEAPR → THERMR round-trip (primary V&V gate)

The MF=7 writer's output is read back with the existing THERMR MF=7 reader
(`crate::thermr::mf7::parse_mf7`) in-memory (`Tape::from_sections`).

- **`endout::tests::inelastic_only_roundtrips_through_thermr`** — writes MT=4 for
  a 3-alpha × 4-beta symmetric law, reads it back, and asserts: AWR survives;
  `LAT`/`LASYM` survive; `B(1) > 0` and `B(3) = A = AWR`; the **alpha and beta
  grids are byte-identical**; and every `S(alpha,beta)` matches the written
  `S_sym = ssm·exp(-beta/2)` to `< 1e-6` relative (7-sig-fig round-trip). The
  trailing effective-temperature TAB1 also survives (`T = 296 K`, `T_eff ≈ 430 K`).
  **Result: round-trip works.**
- **`endout::tests::coherent_elastic_roundtrips_and_is_monotone`** — feeds real
  graphite Bragg edges from `coher` into MT=2, reads back, asserts Bragg energies
  ascending and cumulative `S(E)` non-decreasing and `≥ 0`. **Result: works.**
- **`endout::tests::incoherent_elastic_roundtrips`** — MT=2 `LTHR=2` with two
  temperatures; asserts `SB` and the `(T, W')` table survive and `W'(T)` is
  non-decreasing. **Result: works.**

## Every test + asserted property

| Test | Asserts |
|---|---|
| `coher::graphite_edges_are_monotone_and_nonnegative` | > 10 edges; energies strictly ascending; `S ≥ 0`; forbidden (`S=0`) edges retained as NJOY does; first *allowed* edge in `1–3 meV` (graphite (002)) |
| `coher::all_lattices_produce_ascending_edges` | all 6 moderators yield non-empty ascending-energy edges, `S ≥ 0`, last edge ≤ `emax` |
| `coher::form_factor_graphite_even_odd_branches` | `formf` odd/even-`l3` branches (`(111)=0`, `(300)=4`) |
| `endout::inelastic_only_roundtrips_through_thermr` | MT=4 round-trip (grids, B, S values, T_eff) — see above |
| `endout::coherent_elastic_roundtrips_and_is_monotone` | MT=2 coherent round-trip + monotonicity |
| `endout::incoherent_elastic_roundtrips` | MT=2 incoherent round-trip (`SB`, `W'(T)`) |
| `coldh::add_cold_hydrogen_runs_and_populates_ssp` | para-H2 run: all `ssm`/`ssp` finite and `≥ 0`; `ssm` changed from seed; `ssp` gained positive-beta scattering; normalization finite (self-consistency only, **no** reference oracle) |
| `coldh::add_cold_hydrogen_none_is_noop` | `ColdOption::None` returns `0.0`, no-op |

## Hand-transcribed lattice/material constants — HUMAN RE-VERIFY

These are `data`-statement literals copied by hand from `leapr.f90`; a
transcription slip silently shifts Bragg edges or the cold-H convolution. Please
re-verify against the Fortran source.

**`coher.rs` — lattice constants (`leapr.f90:2508–2531`):**

| Moderator | a [cm] | c [cm] | amsc [amu] | scoh [barn] |
|---|---|---|---|---|
| Graphite | 2.4573e-8 | 6.700e-8 | 12.011 | 5.50 |
| Beryllium | 2.2856e-8 | 3.5832e-8 | 9.01 | 7.53 |
| BeO | 2.695e-8 | 4.39e-8 | 12.5 | 1.0 |
| Aluminium | 4.04e-8 | — | 26.7495 | 1.495 |
| Lead | 4.94e-8 | — | 207.0 | 1.0 |
| Iron | 2.86e-8 | — | 55.454 | 12.9 |

`scoh` is divided by the atom count `natom`. **BeO form-factor constants**
(`formf`, 2939–2941): `c1 = 7.54`, `c2 = 4.24`, `c3 = 11.31`.

**`coldh.rs` — molecular constants (`leapr.f90:1962–1976`, 2000–2010):**

| Symbol | Value | Meaning |
|---|---|---|
| `pmass` | 1.6726231e-24 g | proton mass |
| `dmass` | 3.343586e-24 g | deuteron mass |
| `deh` | 0.0147 eV | H2 rotational constant |
| `ded` | 0.0074 eV | D2 rotational constant |
| `sampch` / `sampih` | 0.356 / 2.526 | H coherent / incoherent amplitudes |
| `sampcd` / `sampid` | 0.668 / 0.403 | D coherent / incoherent amplitudes |
| `amassm` (H / D) | 3.3464e-24 / 6.69e-24 g | molecular mass |

Fixed NJOY-default switches in `coldh`: `ifree = 0`, `nokap = 0`, `jterm = 3`.

## cargo test result (actual)

Command: `bash crates/njoy-outram-park-fork/scripts/test.sh leapr` (12 GB-capped wrapper, release).

```
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 300 filtered out; finished in 1.06s
```

(27 `leapr::*` lib tests, of which 8 are new/changed for this bead: 3 `coher`,
3 `endout`, 2 `coldh`. Baseline before this bead had a `coldh_orchestrator_not_ported`
test that is now replaced by the two real `coldh` orchestrator tests.)

Full crate suite (`scripts/test.sh` = `cargo test --lib --tests --release`, 12 GB-capped):
**exit 0**; `unittests src/lib.rs` = **327 passed; 0 failed** (baseline before this
bead was 274 lib tests; the delta is my 8 new/changed leapr tests plus other fleet
agents' concurrent additions landing in the shared worktree). All 14 test binaries
report `0 failed`; no `FAILED`/`panicked`/`error[...]` anywhere in the run.

Build: `cargo build -p njoy-outram-park-fork --release` — clean, **0 warnings**.

## Notes for reviewers
- All ported code is **untrusted AI draft** (crate policy): the round-trip and
  self-consistency tests verify *implementation* correctness, not *physical
  validation* against a reference NJOY LEAPR tape. Coherent-elastic and cold-H
  results in particular need validation against a known-good MF=7 tape.
- The shared worktree was concurrently edited by other fleet agents during this
  task (mixr, resxsr, groupr, errorr, …); transient whole-crate build breakages
  from their in-progress work were **not** mine and resolved as they landed.
