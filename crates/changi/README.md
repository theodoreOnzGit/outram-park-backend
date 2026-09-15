# CHANGI

**C**onsequence and **H**azard **A**nalysis for **N**uclear **G**round-level and
atmospheric **I**mpacts.

Atmospheric consequences for the OUTRAM PARK suite — *"what happens after
release?"*

## Scope

- Atmospheric dispersion
- Plume transport
- Radionuclide deposition
- Ground contamination
- Radiological consequence assessment
- Dose assessment
- Emergency planning support
- Level 3 PSA support

**Input:** source terms produced by SEMBAWANG (reserved name, not yet created).
Until SEMBAWANG exists, a release-rate time series has to be supplied by hand —
FLEXPART needs nothing more than that from the accident side.

> **The last four scope items are capabilities, not claims of authority.**
> `docs/ecosystem-naming.md` decision 3 draws this line explicitly: *"The
> capability is in scope; that framing is not."* CHANGI may implement
> consequence and dose assessment methods and the machinery an emergency-planning
> or Level 3 PSA study needs. Its outputs must not be presented as authoritative
> for operational emergency response, for dose assessment of real populations, or
> for regulatory Level 3 PSA — see the intended-use limit below, which comes from
> `RESPONSIBLE_USE.md` and is unchanged.

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

Named for Changi, following the workspace's Singapore-place-name convention.

## Intended use — a binding limit, not a disclaimer

**CHANGI is for research, education and verification/validation only.**

Its outputs must **not** be treated as authoritative for operational emergency
response, dose assessment of real populations, regulatory or licensing
decisions, or any safety-critical purpose. This limit comes from the workspace
`RESPONSIBLE_USE.md` (§ intended use, and line 43: outputs "must not be treated
as authoritative for safety, licensing, operational, regulatory, or
emergency-response purposes") and from `docs/ecosystem-naming.md` decision 3
(2026-08-05).

The distinction decision 3 draws is between **capability** and **framing**:
implementing consequence-assessment, dose and Level 3 PSA *methods* is in scope;
presenting the results as fit for operational use is not. Do not reintroduce the
operational framing — an earlier naming draft did, and it was corrected for
exactly this reason.

## Where it sits

CHANGI is the middle link of the offsite chain:

| | Question | Crate |
|---|---|---|
| SEMBAWANG | What gets released? | reserved, not yet created |
| **CHANGI** | **What happens after release?** | **this crate** |
| REDHILL | What happens after deposition? | reserved; will build on `outram-park-fork-pflotran` |

## What exists today

A Rust port of FLEXPART's **surface-layer and deposition scalar kernels** — the
pure functions that turn meteorological surface fields into turbulence scales
and aerosol deposition properties.

| Area | Upstream files | Ported |
|---|---|---|
| Physical constants | `par_mod.f90` | yes |
| Saturation vapour pressure, dynamic viscosity | `ew.f90`, `dynamic_viscosity.f90` | yes |
| Monin–Obukhov similarity, friction velocity, aerodynamic resistance | `psim.f90`, `psih.f90`, `scalev.f90`, `obukhov.f90`, `raerod.f90` | yes |
| Lognormal aerosol bins: settling, Cunningham, Schmidt | `part0.f90` | yes |
| Radioactive decay | `readreleases.f90`, `timemanager.f90` | yes |

**Not ported**, which is most of FLEXPART: the particle advection loop
(`advance.f90`), the Hanna turbulence parameterisation, the convective
boundary-layer scheme, wet scavenging, the Richardson mixing-height diagnostic,
the GRIB/NetCDF readers, the output grids and the OH chemistry. This is the
first verified slice of a port, not "FLEXPART in Rust".

## Verification

Every reference value comes from **compiling and running the upstream FLEXPART
Fortran itself**, at two precisions. Full methodology, results and analysis:
[`docs/flexpart-code-to-code.md`](docs/flexpart-code-to-code.md).

Headline: against a `-fdefault-real-8` build of the same routines, **11 of 12
function groups agree bit-exactly** (the twelfth by one ulp). Against FLEXPART
as it actually ships — which is single precision, since its makefile passes no
`-fdefault-real-8` — agreement is 5e-8 to 3.4e-6, which is upstream's own
precision rather than any translation error.

That is **verification, not validation**: it shows the Rust computes what the
Fortran computes, and says nothing about whether FLEXPART's parameterisations
reproduce measured dispersion.

```bash
cargo test --release -p changi
./dev/build_reference.sh     # regenerate the fixtures (needs gfortran + the upstream clone)
```

## Reuse

Per the workspace "reuse before porting, port before writing" rule:

- **`petir`** supplies `erf` for the aerosol size distribution; FLEXPART's own
  `erf.f90` is deliberately **not** ported.
- **`boon-lay`**'s nuclide database is the intended source of half-lives; this
  crate carries no nuclide data of its own.
- **`outram-mc-libs`**' LCG will supply the pseudo-random numbers the Langevin
  turbulence scheme needs. Not yet wired in — no stochastic code has landed.
- **`outram-foam-basic-lib`** will supply the gridded field and interpolation
  layer when the concentration-grid phase arrives. Deliberately **not** yet a
  dependency: the scalar kernels need nothing from it, and taking a
  finite-volume CFD dependency before there is a field to put on a mesh would be
  premature.

## Licence

GPL-3.0, containing a port of FLEXPART (GPL-3.0-or-later, © FLEXPART 1998-2019).
See [`LICENSE.flexpart`](LICENSE.flexpart) and
[`NOTICE.flexpart`](NOTICE.flexpart). Independent fork; not affiliated with or
endorsed by NILU or the FLEXPART developers.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.
