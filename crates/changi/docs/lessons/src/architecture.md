# Three crates, two ports, one chain

## The offsite chain

OUTRAM PARK splits "what happens after an accident" into questions, one crate
each:

| Crate | Question | Role in this deep dive |
|---|---|---|
| `sembawang` | What gets released? | Hands a source term to `changi` |
| **`changi`** | What happens after release? | **Dispersion, decay in transit, deposition** |
| `buangkok` | What dose does that imply? | A second, independent Gaussian **plume** (from pyDOSEIA), and dose coefficients |

The hand-off from `sembawang` is one function,
`sembawang::chain::pad_for_dispersion`, which pads a source term with zero-release
windows so it partitions a dispersion run from `t = 0`
([`crates/sembawang/src/chain.rs`, lines 27–49](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/sembawang/src/chain.rs#L27-L49)).
`changi` itself computes no part of the source term
([`activity/source.rs`, lines 3–8](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/activity/source.rs#L3-L8)),
and it computes **no dose** of any kind
([`activity/mod.rs`, lines 33–34](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/activity/mod.rs#L33-L34)).

## Inside `changi`: two ports and one layer that is not a port

`changi` holds two independent ports and one module written here
([`src/lib.rs`, lines 57–74](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/lib.rs#L57-L76)):

| Module | What it is | Upstream | Licence | Verified against |
|---|---|---|---|---|
| [`puff`](../../api/changi/puff/index.html) | Analytic Gaussian puff on one wind series | R package `puff` 0.1.1, commit `5213d58` (Hammerling Research Group) | MIT | The upstream R, executed |
| [`flexpart`](../../api/changi/flexpart/index.html) | Kernels of a Lagrangian particle model on gridded meteorology | FLEXPART v10.4, commit `3d7eebf` (NILU) | GPL-3.0-or-later | The upstream Fortran, compiled twice |
| [`activity`](../../api/changi/activity/index.html) | Dilution factors, decay, deposition, survey | **none**, written here | GPL-3.0 | An independent sum and `buangkok`'s plume ([the V&V page](./vv-and-limits.md)) |

### Why the two ports are kept apart

The crate rule is blunt: **do not merge their shared-looking pieces.** Both
have a "stability class" and a "dispersion coefficient", and a common
abstraction would be tempting. But each port is checked against *its own*
upstream, digit for digit, and a shared type would make each comparison harder
to read and easier to break
([`CLAUDE.md`, "Keep them separate"](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/CLAUDE.md#L28-L34)).

`activity` *consumes* both: `puff` for transport and `flexpart::decay` for the
decay weight. It sits above them and defines no type either one uses
([`activity/mod.rs`, lines 44–64](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/activity/mod.rs#L44-L64)).

### Which model actually runs

When `changi::activity` reports a number, **the transport is the `puff`
model, not FLEXPART**
([`activity/mod.rs`, lines 66–76](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/activity/mod.rs#L66-L78)).
`puff` was written for methane leak detection, uses empirical
Pasquill–Gifford sigmas fitted over roughly 0.1–10 km, and has no turbulence
closure. Say which model ran whenever you quote a result.

### What the FLEXPART port is, and is not

FLEXPART releases computational particles, moves them on meteorological
fields, perturbs them with parameterised turbulence, and removes mass by
deposition and decay. The port covers its **numerics**, verified routine by
routine against the compiled Fortran: surface-layer similarity, Hanna
turbulence, the convective boundary layer, dry and wet deposition, the
particle step (`advance`), interpolation, convection, release, output gridding
and the time manager's bookkeeping
([`flexpart/mod.rs`, lines 19–79](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/flexpart/mod.rs#L19-L79)).

What is **not** ported is the I/O: the GRIB and NetCDF readers and the file
writers. The ported numerics take decoded fields as inputs. So the module is
"a verified set of FLEXPART's kernels", not "FLEXPART in Rust", and it cannot
yet be pointed at real meteorological files.

## `buangkok`'s plume: a second, separate Gaussian model

`buangkok` ports **all of pyDOSEIA's computation**, including its Gaussian
plume dilution factors, from
[pyDOSEIA](https://github.com/BiswajitSadhu/pyDOSEIA) at commit `dca4cdc3`
(MIT)
([`buangkok/src/pydoseia/dispersion.rs`, lines 1–28](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L1-L28)).
Its sigma fits are a different set (BARC/AERB) from `changi`'s (Martin / US EPA
ISC), so the two are **not interchangeable and not unified**. That turns out
to be useful: [the V&V page](./vv-and-limits.md) uses the plume as an independent check on the puff
train.

## Where the numbers come from

- **Half-lives and decay constants come from `boon-lay`**, never from a table
  in `changi`, so the two cannot drift
  ([`CLAUDE.md`, lines 83–95](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/CLAUDE.md#L83-L95)).
- **Published HTR-10 nuclide tables** (Liu and Cao 2002: core inventory,
  release rates, accident releases) live in `changi/reference/` with their
  provenance in `changi/docs/References.md`. They are reference data and
  comparison targets, not tuning inputs.
- **`erf` comes from `petir`**: FLEXPART's own `erf.f90` was deliberately not
  ported, because `petir::specfunc::erf` already covers it and the two were
  measured to agree bit for bit
  ([`CLAUDE.md`, lines 77–81](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/CLAUDE.md#L77-L81)).
