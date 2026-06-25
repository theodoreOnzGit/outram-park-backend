# openfoam-appbuilder-lib

Solver application layer for the **OUTRAM PARK** OpenFOAM-in-Rust stack.
Provides solver time loops, polyMesh I/O, case file parsing, and field output.

Depends on:
- `openfoam-basic-lib` — primitives, FV operators, fields, mesh
- `openfoam-turbulence-lib` — turbulence model closures

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

## Status

Scaffold only — no solvers or I/O implemented yet. See `CLAUDE.md` for the
implementation plan and module structure.

## License

GPL-3.0-only (follows OpenFOAM licensing).
