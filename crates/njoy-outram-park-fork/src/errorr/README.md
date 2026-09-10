# ERRORR — multigroup covariance matrices

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §ERRORR); upstream Fortran: `errorr.f90` (~11.2k lines).

## Theory

Evaluators encode their uncertainty about nuclear data as **covariances** — the
joint (relative) covariance matrix of the evaluated quantities. ERRORR reads the
ENDF covariance files and collapses them to a user **multigroup** structure:

| ENDF MF | Covariance of |
|---|---|
| 31 | ν̄ (average fission neutrons) |
| 33 | cross sections |
| 34 | angular distributions |
| 35 | secondary energy spectra |
| 40 | production cross sections |

For cross sections it forms the group relative covariance
`rcov(g, g') = cov(σ_g, σ_{g'}) / (σ_g σ_{g'})` by projecting the ENDF
sub-material covariance components (NC/NI-type sub-subsections, each a pattern of
energy blocks) onto the group structure with the same flux weighting GROUPR uses.
The result feeds S/U analysis (e.g. sandwich-rule Δk/k propagation).

## How the port implements it

The MF=33 cross-section path is **ported end to end and validated against
the NJOY2016 binary** (`run_mf33`, `src/errorr/mf33.rs`). It is the
in-memory equivalent of the deck

```text
errorr / nendf npend 0 nout 0 0 / matd ign iwt iprint irelco / mprint tempin / 0 33 1 1 -1 2e6 0
```

| Fortran (`errorr.f90`) | Rust | What |
|---|---|---|
| `errorr` dictionary scan (l.713-792) | `gridd::scan_reactions` | which MF=33 `MT`s, lumps (851–870), MF=32 present? |
| `gridd` / `merge` / `lumpmt` (l.1091-1768) | `gridd::{gridd, merge, lumpmt}` | covariance energy grid `eni`, derivation coefficients `akxy(iy,ix,k)` over ranges `ek`, lumped components |
| `egngpn` + `uniong` (l.9534-9807) | `groups::neutron_group_structure` + `gridd::uniong` | user bounds `egn`, union grid `un` (`sigfig` to `ndig=6` throughout) |
| `egnwtf` / `egtwtf` / `egtflx` (l.9809-10286) | `weight::{ErrorrWeight, WeightSampler}` | the weight menu `iwt` 1–12 with its panel *stops* (`1.01e`, fusion-peak refinements, tabulated `terpa` steps) |
| `grpav` / `epanel` / `egtsig` + `endf.f90` `gety1` | `grpav::{grpav, PanelState, XsSampler}` | union-group `σ_g` and flux from the PENDF at infinite dilution (threshold shading `0.999999`, discontinuity step `0.999995`) |
| `covcal` / `lumpxs` (l.1770-2417) | `covcal::covcal` | absolute union-group covariances, `LB` 0–6 and 8 decoded with the Fortran's own index arithmetic |
| `sigc` / `covout` (l.7018-7898) | `covout::{sigc, covout, ErrorrResult::to_tape}` | coarse-group `csig`/`cflx`, the `akxy`-weighted collapse with the `isd`/`iabort` fast path, relative/absolute output, and the `nout` tape layout COVR reads |

Two upstream idioms were kept on purpose because the oracle depends on
them: every energy *and* every covariance datum is rounded to 6 significant
figures on input (`sigfig`), and ERRORR's `iwt=6` amplitude is
`wt6b = 1.57855e-3` — not GROUPR's `1.578551e-3` (a genuine
module-to-module difference in NJOY2016).

The earlier structural reader (`covariance.rs`, `read_covariance_section`)
is still available; `covcal` reads the records directly through
`SectionCursor` because it needs them staged in `covcal`'s own flat layout.

### Not ported (returns `NotPorted`, never approximates)

- **MF=32** resonance-parameter covariances (`resprx`/`rpxsamm`/`rescon`,
  the ERRORJ method and the SAMM derivative path). None of the ENDF/B-VIII.0
  tapes in `reference-data/endf/` carries MF=32, so no oracle exists here yet;
  a material with MF=32 is refused rather than run without it.
- **MF=31/34/35/40**, `iread = 1/2` (user reaction lists, extra `MAT1/MT1`
  pairs), `nstan` (ratio-to-standard, `LTY` 1–3, `grist`/`stand`), `nin`
  (input covariance tape merge), `ngout != 0` (`colaps` from a GENDF),
  `covadd`, ENDF/B-IV (`iverf = 4`), the `-33/333` speed-up modes.
- The listing printer (`nsyso` tables); `ErrorrResult` carries everything
  the listing prints.

## Testing

`tests/errorr_mf33_golden.rs` against `reference-data/errorr/` (nine
NJOY2016 runs, decks committed — see that README for provenance):

- **Tier 1 (engine isolation, NJOY's own PENDF in):** H-2 (`iwt=6/irelco=1`
  and `iwt=3/irelco=0`), Be-9, Li-6, C-12, F-19, Si-30, plus env-gated
  U-234 and U-238 (lumped `MT=851/852`, derived `MT=1`/`MT=4`, 135 union
  groups). Every group cross section and every covariance element of every
  block agrees to the tape's printing precision: worst 4.9e-7 relative for
  7-figure fields, 3.6e-6 for the one 6-figure (two-digit-exponent) value.
  Group counts, reaction lists and lumped placeholders match exactly.
- **Tier 2 (crate RECONR + BROADR in):** H-2 and Be-9. Covariances of
  directly-evaluated pairs within 3.3e-6 / 9.0e-4; the thermal-group
  `σ_g` is 32 % / 1.2 % high because the crate's BROADR does not insert grid
  points across the free-gas 1/v rise of a light nuclide (bead `op-tubm`,
  a BROADR defect the oracle exposed — reported, not asserted, here).
- The `to_tape` writer round-trips through the crate's ENDF reader with
  NJOY's layout (`output_tape_round_trips_like_njoy_layout`).
- 46 unit tests (`merge`/`uniong`, `terpa`, `gety1` idioms, every `LB`
  kernel, the weight stops) in the modules' `#[cfg(test)]` blocks.

## Caveats

- **Not required by OpenMC CE** — Phase 5, sensitivity/uncertainty workflows only.
- Positive-semidefiniteness of ENDF covariances is not guaranteed by the data;
  the port reports what the file implies (as NJOY does) and never "fixes" it.
- Infinite dilution only in `grpav` (as upstream with `ngout = 0`).

## References

- NJOY2016 manual §ERRORR (LA-UR-17-20093)
- `errorr.f90` (NJOY2016 2016.79, `ac5adf5`)
- ENDF-102, Files 31–40 covariance formats
