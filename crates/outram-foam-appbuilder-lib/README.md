# outram-foam-appbuilder-lib

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


> **This is OUTRAM PARK's independent Rust translation of selected OpenFOAM®
> algorithms.** It is not the official OpenFOAM® software and is not
> affiliated with, endorsed by, or sanctioned by OpenCFD Ltd. or the ESI
> Group. OpenFOAM® is a registered trademark of OpenCFD Limited — see
> [`TRADEMARKS.md`](./TRADEMARKS.md) for the full attribution and
> non-affiliation notice. Translated from
> [`OpenFOAM/OpenFOAM-dev`](https://github.com/OpenFOAM/OpenFOAM-dev),
> `master` branch — no commit is pinned (translation was done by reading the
> C++ source directly, not from an ongoing codegen-from-clone pipeline); see
> `upstream_source/README.md` for the full provenance record.

Solver application layer for the **OUTRAM PARK** OpenFOAM-in-Rust stack.
Provides solver time loops, polyMesh I/O, case file parsing, and field output.

Depends on:
- `outram-foam-basic-lib` — primitives, FV operators, fields, mesh
- `outram-foam-turbulence-lib` — turbulence model closures

## Planned solvers

| Solver | Description |
|---|---|
| `pimple_foam` | Incompressible transient PIMPLE (pimpleFoam) |
| `rho_pimple_foam` | Compressible density-based PIMPLE (rhoPimpleFoam) |
| `sonic_foam` | Transient compressible psi-based solver (sonicFoam) |
| `rho_central_foam` | Kurganov-Tadmor central-upwind explicit (rhoCentralFoam) |
| `hrm_foam` | Homogeneous Relaxation Model two-phase (HRMFoam) |

## Planned I/O modules

| Module | Description |
|---|---|
| `io::poly_mesh` | polyMesh reader (points, faces, cells, boundary) |
| `io::control_dict` | controlDict parser (time control, I/O settings) |
| `io::fv_schemes` | fvSchemes parser (numerical scheme selection) |
| `io::fv_solution` | fvSolution parser (linear solver + PIMPLE control) |
| `io::output` | OpenFOAM ASCII field writer and VTK export |

## GeN-Foam port (`genfoam`)

This crate is also the in-workspace home for the Rust port of
[GeN-Foam](https://gitlab.com/foam-for-nuclear/GeN-Foam) (Generalized Nuclear
Foam), an OpenFOAM-based reactor-multiphysics solver (neutronics +
thermal-hydraulics + thermo-mechanics), GPL-3.0, upstream commit `652b3da`. The
port lives under `src/genfoam/`, cleanly separated from the OpenFOAM solver
ports above. GeN-Foam's neutronics is deterministic and self-contained here — it
does **not** depend on the NJOY / Monte Carlo data crates.

The full module map and dependency-ordered translation plan are in
[`docs/genfoam-port-plan.md`](./docs/genfoam-port-plan.md). This is an
incremental, multi-session effort (~88k LOC of upstream physics).

| `genfoam` module | Status |
|---|---|
| `neutronics::point_kinetics` | **Implemented** — 0-D point-kinetics ODE core (backward-Euler implicit solve), reactivity- and external-source-driven. Verified against the analytical inhour equation (asymptotic period matches to ~0.007 %; see `tests/genfoam_point_kinetics_inhour.rs`). Mesh/feedback/GEM/FMU/liquid-fuel coupling deferred. |
| `neutronics::{xs, diffusion, sp3, sn}`, base state | Planned |
| `common` (timeProfile, InterpolateTable) | Planned |
| `multi_region`, `thermal_hydraulics`, `thermo_mechanics` | Planned |

## Status

OpenFOAM solver loops and I/O: scaffold (see `CLAUDE.md`). GeN-Foam port: first
verified slice (`genfoam::neutronics::point_kinetics`) landed; everything else
planned (see `docs/genfoam-port-plan.md`).

## License

GPL-3.0-only (follows OpenFOAM and GeN-Foam licensing).
