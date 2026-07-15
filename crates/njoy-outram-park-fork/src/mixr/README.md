# MIXR — linear combinations of cross sections

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §MIXR); upstream Fortran: `mixr.f90`.
>
> **Provenance.** Ported from NJOY2016 `src/mixr.f90`, git commit
> `ac5adf5f33d893e42f2eed7fb286b0d51c7580da`. NJOY2016 is under a modified BSD
> 3-Clause (LANL/DOE) licence, GPL-compatible; this derivative is distributed
> under GPL-3.0-only. Modified, non-LANL version, not endorsed by LANL/DOE. See
> crate root `LICENSE.njoy` + `NOTICE`.

## Theory

MIXR builds a new PENDF tape whose reactions are user-specified **linear
combinations** of cross sections drawn from one or more input tapes:

$$\sigma_{\text{out},\,MT}(E) = \sum_i w_i \, \sigma_{\text{in}_i,\,MT}(E)$$

The classic uses are (a) constructing an **element** cross section from its
isotopes weighted by natural abundance, and (b) mixing materials to plot combined
cross sections (with PLOTR/VIEWR). The output tape contains only ENDF **File 1
(MT=451) and File 3** sections and assumes **linear-linear** interpolation
(`mixr.f90:22-29`).

### Algorithm (`mixr.f90:257-390`)

For each requested output MT:

1. Locate the MF=3 section for that MT on each contributing input material
   (`mixr.f90:261-289`). A missing section is a warning, not an error — that
   component simply contributes nothing (`mixr.f90:283-287`).
2. Unionise the tabulated **energy grids** (eV) of the contributing cross
   sections (`mixr.f90:291-301`).
3. At every union energy `E`, evaluate each contributing cross section (barns),
   interpolating with its own ENDF interpolation law, and form the weighted sum
   (`mixr.f90:296-303`). Out-of-range conventions follow `gety`: **0 below** an
   input's grid (`mixr.f90:509-512`), **held constant** at the last value
   **above** it (`mixr.f90:514-517`).
4. Write the result as a single lin-lin (INT=2) MF=3 TAB1, cross sections
   rounded to 7 significant figures (`mixr.f90:319-370`, `sigfig` at
   `mixr.f90:350`).

## What is ported

| Piece | Status | Location |
|---|---|---|
| Six-card input deck (`mixr.f90:31-56,99-121`) | **Ported** | `input.rs` — `MixrInput`, `MixComponent`, `MixrInput::from_cards` |
| `gety` value retrieval + out-of-range rules (`mixr.f90:392-530`) | **Ported** | `mix.rs` — `gety_value` |
| Union-grid weighted sum (`mixr.f90:291-313`) | **Ported** | `mix.rs` — `mix_reaction` (returns exact `(E, σ)` points) |
| `sigfig` 7-figure rounding (`util.f90:361-393`) | **Ported** | `mix.rs` — `sigfig` |
| Full tape assembly: MF=1/451 + MF=3 sections (`mixr.f90:196-378`) | **Ported** | `mix.rs` — `mix` / `MixrInput::mix` |
| File-level card-deck driver (`nsysi`→`nout`) | **NotPorted** | `mod.rs` — `run()` returns `NjoyError::NotPorted`, as `crate::moder::run` does |

The mixing physics is complete and drivable end-to-end in memory:
`Tape::read` → `MixrInput::mix(&[Tape])` → `Tape` → `Tape::write`. Only the
Fortran card-reader shell is deferred.

## Fidelity notes (documented divergences from genuine NJOY)

- **Union grid = distinct tabulated energies.** Duplicate-energy step
  discontinuities are collapsed (the crate's `eval_tab1` returns one value per
  energy). Exact for lin-lin PENDF, MIXR's main use.
- **`mix_reaction` returns the exact (unrounded) weighted sum** so the
  arithmetic is testable; `mix` applies `sigfig(σ, 7, 0)` only when laying the
  values into the output tape (matching `mixr.f90:350`).
- **MF=1/451 comment text is not preserved.** The crate's `[f64; 6]`
  section-row model cannot store Hollerith characters, so the card-6
  description becomes a blank comment record. AWI/EMAX/NSUB header fields use
  MIXR's default seeds (`mixr.f90:147-149`: AWI=1, NSUB=10) with EMAX taken
  from the produced grids, rather than being read back from the first input's
  MF=1 header.
- **`uom` is not used at this boundary.** Energies (eV), cross sections
  (barns), and weights (dimensionless) are plain `f64`, consistent with the
  `crate::endf` tabulated model the engine reuses; units are spelled out in
  every doc comment.

## Testing

Inline `#[cfg(test)] mod tests` in `mix.rs`. Run via
`crates/njoy-outram-park-fork/scripts/test.sh mixr` (12 GB cap).

**Status: 9/9 unit tests + 2/2 doctests pass (2026-07-15).**

| Test | Methodology | Result |
|---|---|---|
| `sigfig_rounds_to_seven_figures` | `sigfig(1.23456789,7,0)` vs 7-figure round (`util.f90:361-393`) | ≈1.234568; 0→0 |
| `gety_value_boundaries_match_fortran` | grid (1,10)→(3,30) lin-lin; below/mid/at-max/above-max | 0 / 20 / 30 / 30 (held) |
| `identity_single_input_weight_one` | one component, weight 1 (`mixr.f90:302`); σ=(5,7,11)b | output == input |
| `half_and_half_is_average_on_shared_grid` | weights (0.5,0.5) on shared grid → average | (1,5)(2,30)(3,40) |
| `union_grid_interpolates_disjoint_energies` | grids {1,3} & {2,4}, weights 1,1; union {1,2,3,4} | (1,10)(2,120)(3,180)(4,230) |
| `additivity_weighted_sum` | weights (2,3) = 2A+3B pointwise | (1,32)(2,64) |
| `zero_where_one_input_absent_below_range` | component = 0 below its grid; union {1,2,3} | (1,10)(2,120)(3,320) |
| `mix_produces_roundtrippable_tape` | full `mix` → MF=1/451 + MF=3; TAB1 round-trip + `Tape::write` | sections present; σ=(20,30); serialises |
| `missing_reaction_yields_empty_grid` | MT present on no input (`mixr.f90:283-287`) | empty grid |

**Not yet done (future V&V):** validation against genuine NJOY MIXR output on a
real multi-isotope PENDF (e.g. natural-element reconstruction), which would
exercise real interpolation regions and the `sigfig`'d tape byte layout. The
current tests verify the *arithmetic* and *out-of-range logic* against
hand-computed references, not a Fortran golden file.

## Caveats

- Convenience/plotting utility (Phase 6 in the porting plan).
- Output is MF=3 only and lin-lin — not a full evaluation; do not feed it back
  into resonance-dependent modules.
- AI-assisted draft — untrusted until human-reviewed per the workspace V&V
  workflow.

## References

- NJOY2016 manual §MIXR (LA-UR-17-20093)
- `mixr.f90` (NJOY2016, git `ac5adf5f`)
- `util.f90:361-393` (`sigfig`)
