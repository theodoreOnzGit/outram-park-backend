# Upstream provenance — PFLOTRAN

This directory records the provenance of the upstream **PFLOTRAN** project that
`outram-park-fork-pflotran` translates. It holds no PFLOTRAN source yet; a
reference clone (kept here for codegen/provenance only, mostly gitignored) will
be added when translation begins.

## Upstream

| Field | Value |
|---|---|
| Project | PFLOTRAN — subsurface flow & reactive transport |
| Home | https://www.pflotran.org |
| Code | https://bitbucket.org/pflotran/pflotran |
| Docs | https://documentation.pflotran.org |
| Stewardship | LANL, PNNL, ORNL, LBNL, SNL (US DOE national labs) |
| Language | Fortran (2003+) on PETSc (MPI) |
| License | GNU LGPL, version 2.1 or later (LGPL-2.1-or-later) |

## License determination (2026-07-20)

The upstream license was determined to be **LGPL-2.1-or-later** from the
PFLOTRAN documentation and the OSTI / DOECODE records for the code. The raw
upstream `LICENSE`/`COPYRIGHT` file was **not** reachable from the environment
this record was written in, so this is a documentary determination, not a
byte-for-byte reading.

**Action required before publish or before any code is ported (bead op-v6s.1):**

1. Clone upstream PFLOTRAN.
2. Read its `LICENSE` / `COPYRIGHT` verbatim; confirm LGPL-2.1-or-later (vs a
   bare LGPL-2.1, LGPL-3.0, or a mixed-file situation) and record the exact
   header text and any per-file copyright lines here.
3. Confirm GPL-3.0 compatibility of the confirmed license (LGPL-2.1-or-later →
   GPL is permitted by LGPL-2.1 §3; re-verify if the finding differs).
4. Capture the clone's commit SHA and date accessed.

## Recommended citation

Cite the PFLOTRAN user manual / theory guide when using derived functionality,
e.g. Lichtner, P.C., Hammond, G.E., Lu, C., Karra, S., Bisht, G., Andre, B.,
Mills, R.T., Kumar, J. (2015), *PFLOTRAN User Manual*, and the appropriate
methods papers for the specific process models translated. Confirm the current
recommended citation against the upstream `documentation.pflotran.org` before
citing in a publication.
