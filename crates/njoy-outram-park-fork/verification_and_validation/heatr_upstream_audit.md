# HEATR against upstream NJOY2016: routine-by-routine audit (2026-10-05)

**Scope.** Every routine of `heatr.f90` (NJOY2016 `ac5adf5f33`, 6 322 lines,
33 routines) against this crate as it stood on 2026-10-05 after GitHub #535's
H6a and H6b part 1 (`develop` `35e7146f2`). The question asked of each
routine: does the crate do what upstream does, on the same inputs, through the
same control flow? "Partial" means some of the behaviour exists somewhere in
the crate but not as upstream does it, or not reachable from a HEATR run.

**Method.** The whole of `heatr.f90` was read. For each routine, the crate was
searched for the domain noun and the upstream name (`src/heatr/`,
`src/photon/`, `src/acer/photon_blocks.rs`, `src/groupr/file4.rs`), and what
was found was read against the Fortran.

**Headline.** The crate had **no HEATR**. It had physics pieces that
approximate parts of `nheat` and `gheat` on a different grid, but nothing that
takes HEATR's inputs (an ENDF tape, a PENDF tape, the four input cards) and
produces HEATR's output (the PENDF with MF=3/MT=301 and the requested partial
kermas and damage MTs, and a revised directory). `heatr::run()` returned
`NotPorted`.

## Routine table

| upstream routine | lines | crate before this audit | status |
|---|---|---|---|
| `heatr` (driver: cards 1-5, `npk` bookkeeping, `kchk`, temperature loop, default `ed`) | 47-415 | `heatr::run()` returns `NotPorted`; `damage::default_displacement_energy` holds the `ed` table | **missing** (table present) |
| `horder` | 416-436 | — | **missing** |
| `hinit` (dictionary flags, MT=458 fission Q, MF=6 recoil checks, `hconvr` call, union grid from MT=1) | 437-983 | the skip-list flags only, inside `Kerma::from_endf` | **partial**: no MT=458, no MF=6 scan (`mt6no`, `i6p`, `mgam`), no `nmiss4`, grid is a union of RECONR sections rather than `hinit`'s MT=1 walk |
| `nheat` | 984-1665 | `Kerma::from_endf` (two-body `disbar`, `q0` table, skip list); H4/H5 kinematic estimates | **partial**: no MF=6 subsection loop or generated recoil (`irec`), no `ebal6`, no MT=458 fission Q (polynomial or tabulated), no `nqa`/`qbar`, no sequential (n,2n) MT=46-49, no partial kermas, no `kchk` limits, no damage for any channel but two-body |
| `indx` | 1666-1740 | — | **missing** (no partial kermas) |
| `capdam` | 1741-1828 | — | **missing** |
| `disbar` | 1829-2014 | `heatr::twobody::DisbarWalker` (mean energy only) | **partial**: `dame` (64-point Gauss-Legendre damage with MF=4) missing; `edis` elastic damage threshold missing |
| `df` | 2015-2053 | `damage::lindhard_damage` | **partial**: exact `2/3` etc. where upstream uses `.666666667`; upstream's saved constants (the last `e = 0` call's `zr, ar, zl, al` are used by every later call) not reproduced |
| `conbar` | 2054-2298 | `spectra::EmissionSpectrum::mean_energy` for MF=5/MF=6 means (H5) | **partial**: not `conbar`'s subsection weighting, no `hgtyld` fission yield, no damage, no (n,np)-as-(n,p) damage |
| `hgtyld` | 2299-2399 | `NuBar::from_endf` (MF=1/452) | **partial**: not the MT=456 choice, not MT=455 |
| `anabar`, `r`, `rerfc` | 2400-2500 | partly in `spectra.rs` | **partial** |
| `anadam`, `sed` | 2501-2615 | — | **missing** |
| `tabbar` | 2616-2707 | — | **missing** (law < 0 MF=6 form too) |
| `tabdam` | 2708-2754 | — | **missing** |
| `sixbar` | 2755-3072 | — | **missing** for neutrons and charged particles; MF=6 photons have a law-1 mean in `photon::mf6` |
| `getsix` | 3073-3436 | `photon::mf6` (photons, law 1 lab only) | **partial** |
| `h6cm`, `h6ddx`, `h6dis`, `bacha`, `h6psp` | 3437-4134 | — (GROUPR has its own CM integrator, `groupr::kinematics::cm`) | **missing** |
| `tabsq6` | 4135-4213 | — | **missing** (H6c) |
| `hgtfle`, `hgetco` | 4214-4552 | `groupr::file4::File4Angular` with `File4Options::HEATR` | **partial**: isotropy returns `1e10` not `etop`; no LTT=3 switch at the last energy; no `ehi + ehi/100` extension; `iso` set from LI only |
| `hconvr` | 4553-5045 | `acer::photon_blocks::Lo2Cascade` (yields) | **partial**: no MT=456 insertion, no MF=14 rewrite, no MF=3/MF=12 MT continuity checks |
| `hgam102` | 5046-5070 | — | **missing** |
| `gheat` | 5071-5449 | `photon::PhotonProduction` + `photon::capture` | **partial**: equivalent MT=442 (verified at print precision on Fe-58, Si-28), but not on HEATR's grid, no partial kermas, no 443 `hk`, no capture damage, no energy-check listing |
| `gambar`, `tabsqr`, `disgam` | 5450-5688 | `photon::capture` (`gambar_interp`, `tabsqr`, `recoil_mass_energy`) | **partial**: recoil only, no damage (`esqd`, `edam`) |
| `hout` | 5689-6322 | — | **missing**: no MF=3 output, no directory revision, no listing, no plot file |

## Plan that follows from it

Translate the module as upstream structures it, one Rust function per
routine, with in-memory sections in place of the scratch tapes (`iold`,
`inew`, `nscr`, `nend4`, `nend6`) and each routine's Fortran `save` state held
in an explicit struct. Reuse what is already a faithful port of a shared
utility: `endf::gety1::Gety1`, `endf::interp::{terp1, terpa}`, `mixr::mix::sigfig`,
`groupr::kinematics::shared::legndr`, `common::phys`. Then test the whole
against NJOY2016's own HEATR tapes.

The existing `Kerma`, `DamageEnergy` and `PhotonProduction` are left in place
until the port is tested: they are what the ACE route uses today.

## Result (2026-10-05)

The plan was carried out: every routine in the table above now has a
translation in `src/heatr/driver/` (`heatr::heatr`), including the ones marked
**missing** or **partial** here, which describe the state *before* the port.
Tested as planned, against NJOY2016's own HEATR output tapes: **byte-identical
on all 62 neutron evaluations** in `reference-data/endf/` at `local = 0`
and at `local = 1, iprint = 2`, and on 8 committed regression decks that also
compare the `viewr` plot file and the listing. Methodology and numbers:
`heatr_vs_njoy2016.md` §6. ~~The existing `Kerma` … are what the ACE route uses
today.~~ The ACE route now uses `heatr::heatr_kerma`; `Kerma`, `DamageEnergy`
and `PhotonProduction` remain as the reduced-order typed API.

Deliberate divergences from upstream, all of which raise an error where
upstream would carry on silently:

- `sixbar` returns an error if the energy walk asks for more records than
  a subsection holds. Upstream would read the next subsection's records.
- Card or tape errors are `NjoyError` values, not `error()` aborts.

None of these was triggered by any of the 62 evaluations.
