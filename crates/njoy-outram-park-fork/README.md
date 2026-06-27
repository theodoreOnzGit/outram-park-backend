# njoy-outram-park-fork

Pure-Rust port (**work in progress**) of [NJOY2016] — the modular nuclear-data
processing system that turns evaluated ENDF data into libraries for transport
codes. In OUTRAM PARK its job is to produce the **ACE** continuous-energy
libraries that [`openmc-libs`] consumes: NJOY is the data-prep step *upstream* of
an OpenMC calculation.

> **Status:** scaffold. No module produces real output yet. The module entry
> points return `NjoyError::NotPorted`. See [`docs/porting-plan.md`](docs/porting-plan.md).

## License and provenance — please read

This crate is a **derivative work** (a translation) of NJOY2016 v2016.79.

- **Upstream license:** NJOY2016 is under a *modified BSD 3-Clause* license (the
  LANL/DOE variant), preserved verbatim in [`LICENSE.njoy`](LICENSE.njoy). Its
  terms continue to apply to everything derived from NJOY2016.
- **This crate's license:** `GPL-3.0-only`, matching the rest of the OUTRAM PARK
  workspace. The modified BSD 3-Clause license is GPL-compatible, so the combined
  work may be distributed under the GPL.
- **Not the LANL version.** This is **not** endorsed by or affiliated with Los
  Alamos National Laboratory, LANL, Los Alamos National Security LLC, or the U.S.
  Government. Do **not** report issues with this port to the NJOY developers.

The full provenance, modification statement, and no-endorsement notice are in
[`NOTICE`](NOTICE). Redistributions must keep both `LICENSE.njoy` and `NOTICE`.

## The pipeline

```
MODER → RECONR → BROADR → [HEATR] → [GASPR] → [PURR] → [THERMR] → ACER → ACE file → OpenMC
```

Modules in `[brackets]` are optional depending on what physics the ACE library
needs (heating/damage, gas production, unresolved-resonance probability tables,
thermal scattering).

## Verifying against upstream

The reference Fortran NJOY2016 lives at `../../../NJOY2016` and is used as a
golden oracle: run a module in upstream NJOY on a reference ENDF evaluation,
then assert the Rust port reproduces the same tape/ACE output within tolerance.
See the porting plan for the test strategy.

[NJOY2016]: https://github.com/njoy/NJOY2016
[`openmc-libs`]: https://github.com/theodoreOnzGit/outram-park-backend
