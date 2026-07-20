# GAMINR — multigroup photoatomic data

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §GAMINR); upstream Fortran: `gaminr.f90` (1517 lines), git
> commit `ac5adf5f33d893e42f2eed7fb286b0d51c7580da`.

## Theory

GAMINR is the photon analogue of GROUPR: it produces complete multigroup
**photoatomic** (photon–electron-cloud interaction) data from ENDF photoatomic
evaluations. The photon interaction is decomposed into:

- **coherent (Rayleigh)** scattering — no energy loss, angular redistribution set
  by the atomic **form factor** F(q, Z);
- **incoherent (Compton)** scattering — energy + angle from the Klein–Nishina
  cross section modulated by the **incoherent scattering function** S(q, Z), which
  accounts for electron binding;
- **pair production** — above 1.022 MeV, in the nuclear and electron fields;
- **photoelectric absorption** — with fluorescence.

Cross sections are group-averaged over a photon group structure and weight, and
the coherent/incoherent parts get **Legendre group-to-group scattering matrices**.
The initial-energy quadrature is identical to GROUPR's; the secondary
energy-angle quadrature uses Gaussian (Lobatto) integration.

## What is ported vs NotPorted (this pass)

This pass ports the **self-contained, testable front end** of GAMINR — the
input-card model, the built-in group structures, and the built-in weight
functions — plus a driver skeleton. The numeric photon-interaction engine is
marked `NjoyError::NotPorted` rather than fabricated.

| Piece | Fortran (`gaminr.f90`) | Rust | Status |
|---|---|---|---|
| Input card deck (cards 1–7) | `ruing` 538–583; card summary 37–92 | `input.rs` (`GaminrInput`, selector enums) | **Ported** |
| Standard "process all" reaction set | `mflst`/`mtlst`/`mtlst6`/`nmlst` 111–118 | `input::standard_reactions` | **Ported** |
| Photon group structures (`igg` 0,2–10) | `genggp` 585–776 | `photon_groups.rs` (`PhotonGroupStructure`) | **Ported** |
| Read-in group grid (`igg=1`) | `genggp` 675–679 | `input::GroupGrid` (capture only) | **Partial** (captured/validated; boundary read from deck) |
| Weight: constant (`iwt=2`), 1/E+rolloffs (`iwt=3`) | `gnwtf` 778–823; `gtflx` 825–872 | `weights.rs` (`WeightOption`, `PhotonWeight`) | **Ported** (in-range TAB1 eval) |
| Weight: read-in TAB1 (`iwt=1`) | `gnwtf` 803–805 | — | **NotPorted** |
| Cross-section retrieval | `gtsig` 1133–1160 | — | **NotPorted** (needs ENDF `gety1`/`findf`) |
| Feed functions (coherent/incoherent/pair) | `gtff` 1162–1514 | — | **NotPorted** |
| Panel quadrature (group-constant integrals) | `gpanel` 874–1011 | — | **NotPorted** |
| Group-constant display / averaging | `dspla` 1013–1131 | — | **NotPorted** |
| ENDF tape control flow + GENDF writer | `gaminr` 133–536 | `run_with_input` skeleton | **NotPorted** (documented) |

### `igg` photon group-structure map (`gaminr.f90:73–86, 585–602`)

| `igg` | structure | groups |
|------:|-----------|-------:|
| 0  | none | 0 |
| 1  | arbitrary (read from input deck) | — |
| 2  | CSEWG | 94 |
| 3  | LANL | 12 |
| 4  | Steiner (ORNL-TM-2564) | 21 |
| 5  | Straker | 22 |
| 6  | LANL | 48 |
| 7  | LANL | 24 |
| 8  | VITAMIN-C | 36 |
| 9  | VITAMIN-E | 38 |
| 10 | VITAMIN-J | 42 |

`eg2`–`eg6` and `eg8` are written in MeV in the Fortran and scaled by `1.0e6`
into eV here; `eg7` and `eg10` are already in eV. VITAMIN-C (`igg=8`) is
VITAMIN-E's table with `eg8(7)` (0.075 MeV) and `eg8(39)` (20 MeV) removed.

**Local-copy note:** GAMINR carries its own photon group tables inside `genggp`;
they are ported locally here (not shared with GROUPR's photon structures) to keep
this port faithful to `gaminr.f90` and avoid reaching into a concurrently-edited
`src/groupr/`. The lead may dedupe later if the tables prove identical.

## Provenance

- **Upstream:** `NJOY2016/src/gaminr.f90`, 1517 lines, git commit
  `ac5adf5f33d893e42f2eed7fb286b0d51c7580da`.
- **Licence:** NJOY2016 is modified BSD 3-Clause (LANL/DOE), GPL-compatible; this
  derivative is GPL-3.0-only. Modified, non-LANL version, not endorsed by
  LANL/DOE. See crate-root `LICENSE.njoy` + `NOTICE`. Every ported `.rs` file
  carries the provenance header.

## Testing

Inline `#[cfg(test)]` unit tests (run via `scripts/test.sh gaminr`). Methodology
and real numbers are in each test's doc comment. Verified 2026-07-15 against the
`gaminr.f90` DATA statements:

- **Group counts / boundary counts** (`photon_groups::tests::group_counts_match_fortran`):
  `igg` 2→95, 3→13, 4→22, 5→23, 6→49, 7→25, 8→37, 9→39, 10→43 boundaries
  (each = `ngg+1`).
- **Monotonicity** (`boundaries_strictly_ascending`): all 9 built-in structures
  strictly ascending in eV.
- **Endpoints** (`endpoints_match_fortran`): CSEWG 5.0e3 … 2.0e7 eV; LANL-24
  1.0e4 … 3.0e7 eV; VITAMIN-J 1.0e3 … 5.0e7 eV.
- **VITAMIN-C derivation** (`vitamin_c_drops_two_bounds`): 37 bounds; 7.5e4 eV
  and 2.0e7 eV present in `igg=9`, absent in `igg=8`.
- **Weight tables** (`weights::tests`): `iwt` selector round-trips; constant
  weight = 1.0; built-in 1/E table = `(1e3,1e-4),(1e5,1),(1e7,1e-2),(3e7,1e-4)`;
  log-log interpolation exact at nodes, clamped outside, geometric-mean midpoint.
- **Reaction set** (`input::tests::standard_reaction_set`): 9 reactions, `mflst`
  and `mtlst`/`mtlst6` match; ENDF-VI switches MT 602/621 → 522/525.
- **Driver skeleton** (`mod::tests`): `run()` → `NotPorted("gaminr")`;
  `run_with_input` reaches the numeric-engine `NotPorted` for a valid deck and
  fails validation for an `igg=1` deck lacking a grid.

Test status at authoring: see the porting sub-agent hand-off for the exact
`scripts/test.sh gaminr` pass/fail counts. These unit tests exercise the ported
front end only — they are **not** a physics V&V against the Fortran numeric
oracle (that gate awaits the `gtff`/`gpanel` engine).

## Caveats

- **Untrusted AI draft.** Front-end only; the numeric group-averaging engine is
  absent. Do not treat any group-averaged cross section as produced.
- **Not required by OpenMC CE neutron transport** — Phase 5.
- Needs photoatomic evaluations (a different sublibrary from neutron ENDF).
- The 1/E weight evaluation reproduces in-range ENDF log-log TAB1 interpolation;
  it does **not** reproduce NJOY `terpa`'s out-of-range/discontinuity handling or
  the `gtflx` panel-subdivision limiter (`step=1.05`).
- Fluorescence/relaxation cascades are an evaluation-data concern, not modelled
  here beyond what the photoelectric cross section carries.

## References

- NJOY2016 manual §GAMINR (LA-UR-17-20093)
- `gaminr.f90` (NJOY2016, commit `ac5adf5f33d893e42f2eed7fb286b0d51c7580da`)
- Hubbell et al., atomic form factors & incoherent scattering functions
