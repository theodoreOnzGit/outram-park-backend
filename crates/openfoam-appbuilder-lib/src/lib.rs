// openfoam-appbuilder-lib
//
// Solver application layer for the OUTRAM PARK OpenFOAM-in-Rust stack.
// Depends on openfoam-basic-lib (primitives + FV operators) and
// openfoam-turbulence-lib (turbulence model closures).
//
// Planned modules (not yet implemented):
//
// io/
//   poly_mesh    — polyMesh reader: points, faces, cells, boundary patches
//   control_dict — controlDict parser: startTime, endTime, deltaT, writeInterval, …
//   fv_schemes   — fvSchemes parser: time/gradient/divergence/laplacian scheme selection
//   fv_solution  — fvSolution parser: linear solver settings (PCG, PBiCGStab, GAMG, …)
//   output       — OpenFOAM field writer and VTK export
//
// solvers/
//   pimple_foam       — incompressible transient PIMPLE loop (pimpleFoam)
//   rho_pimple_foam   — compressible density-based PIMPLE (rhoPimpleFoam)
//   sonic_foam        — transient compressible sonicFoam (psi-based)
//   rho_central_foam  — density-based central-upwind Kurganov-Tadmor (rhoCentralFoam)
//   hrm_foam          — Homogeneous Relaxation Model two-phase flow (HRMFoam)
//                       Reference C++ source: ../HRMFoam/ (outside this workspace)
