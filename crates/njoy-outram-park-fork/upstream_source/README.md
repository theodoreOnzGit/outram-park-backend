# Upstream source

- **Project:** NJOY2016
- **Repository:** <https://github.com/njoy/NJOY2016>
- **License:** Modified BSD 3-Clause (LANL/DOE variant)
- **Branch tracked:** `master`
- **Commit at last sync:** `ac5adf5f33d893e42f2eed7fb286b0d51c7580da` (version 2016.79)
- **Clone command:** `git clone https://github.com/njoy/NJOY2016.git upstream_source/NJOY2016`

## Provenance

`njoy-outram-park-fork` is a pure-Rust translation (line-traceable port) of
NJOY2016 — the nuclear-data processing system (RECONR/BROADR/THERMR/ACER and
related modules) originally written in Fortran 90 by Los Alamos National
Laboratory. ~120,000 lines of Fortran across 39 source files are the porting
target; see `docs/porting-plan.md` for the full module list, the
Fortran→Rust file map, and the phased porting order.

The clone at `upstream_source/NJOY2016/` also carries NJOY2016's own `tests/`
directory (large golden-output regression data, ~1 GB) — this is used
directly as the **verification oracle**: each translated Rust module is
checked against NJOY's own reference outputs for the same inputs (see the
root `CLAUDE.md`'s mandatory V&V documentation rule, and `docs/porting-plan.md`'s
golden-file verification strategy).

## Licensing note

NJOY2016 is under a modified BSD 3-Clause license (LANL/DOE variant), which
is GPL-compatible — `njoy-outram-park-fork` as a whole is `GPL-3.0-only`
(the OUTRAM PARK workspace default). This is a **derivative work**, not the
LANL-distributed version, and is not endorsed by, affiliated with, or
supported by Los Alamos National Security, LLC, Los Alamos National
Laboratory, LANL, or the U.S. Government. `LICENSE.njoy` and `NOTICE` at the
crate root carry the verbatim upstream copyright/disclaimer and must never be
altered or removed — see `CLAUDE.md`'s "License compliance" section (marked
mandatory) for the full set of compliance requirements.

---

The actual clone lives at `upstream_source/NJOY2016/` and is **gitignored**
(see `.gitignore`) — never committed, present for development only (it is
large: source is ~3.6 MB, but the bundled `tests/` golden-output data is
~1 GB). AI agents doing porting/verification work against NJOY2016 should
clone it fresh if not already present:

```bash
git clone https://github.com/njoy/NJOY2016.git upstream_source/NJOY2016
```
