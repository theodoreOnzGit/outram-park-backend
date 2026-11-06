# Variable flow area in `HemPipe` (gh:#1058)

> Research, education and V&V only. AI draft, unreviewed.

Part of the RELAP-7 parity epic (gh:#1056). The quasi-1-D equations of the
RELAP-7 Theory Manual §2.1.1 (Eqs 10–23; cited, not quoted), in finite-volume
form:

```text
V_i dW_i/dt = −(F A)_{i+½} + (F A)_{i−½} + [0, p_i (A_{i+½} − A_{i−½}), 0],
V_i = Δx A_i,  A_i = (A_{i−½} + A_{i+½})/2
```

`F` is the existing KNP face flux per unit area. The wall-pressure force uses
the cell pressure. Code: `src/hem/pipe.rs` (`AreaProfile`,
`HemPipeGeometry::with_area_profile`, `flow_area_at`, `hydraulic_diameter_at`,
`wall_area_per_length_at`). Tests: `tests/hem_pipe_variable_area.rs`.

**Scope of this record: `HemPipe` only.** Variable area in the `tampines`
drift-flux and two-fluid solvers (`multiphase_1d`) is not done here; see
gh:#1058.

`AreaProfile` is a `Copy` enum of analytic profiles (uniform, linear,
converging–diverging), so `HemPipeGeometry` stays `Copy`. A tabulated profile
would need heap storage and is not offered.

## Predictions (written before the first run, 2026-11-01)

1. Uniform area: byte-identical to the constant-area solver.
2. Fluid at rest at uniform pressure in a nozzle stays at rest.
3. Subsonic converging–diverging nozzle: pressure within 1 % of the
   isentropic area–Mach solution; mass flux uniform to < 1 %.
4. Shocked nozzle: shock within 3 cells of the analytic position, post-shock
   pressure within 3 %.
5. Mass and total energy conserved to round-off between walls.

## Results (2026-11-01, release build)

| # | Case | Measured | Prediction |
|---|---|---|---|
| 1 | Sod (ideal gas, 100 cells) and IF97 choked break (7 MPa, 40 cells), hashed over every cell state, the total mass and the outflow, on the old and new code | identical hash `c79c9553e38e985d` | held |
| 2 | Uniform 1e5 Pa at rest, walls, 200 steps | max \|u\| = 1.8e-13 m/s | held |
| 3 | Subsonic nozzle (A 1.5 → 1.0 → 1.5 m², 1 m, 200 cells, p₀ 1e5, T₀ 300 K, p_e 0.95e5), 200 ms | worst pressure error 0.31 %, mass-flux spread 0.74 % | held |
| 4 | Shocked nozzle, p_e/p₀ = 0.75, 100 ms | shock at 0.8650 m against analytic 0.8604 m (+0.92 cells; M₁ = 1.612); post-shock pressure −0.13 % | held |
| 5 | Riemann problem in the nozzle, walls, 2 ms | mass change 6.7e-16, energy change 4.4e-16 | held |

Why the uniform case is exact: the face-to-cell area ratios are exactly 1.0
and the wall force exactly 0.0. The total mass and energy keep the original
`(Σq)·V` sum when the profile is uniform.

**Grid study, subsonic nozzle, 200 ms:**

| Cells | Worst pressure error | Location |
|---|---|---|
| 200 | 0.31 % | exit cell |
| 400 | 0.18 % | throat |
| 800 | 0.18 % | throat |

### Corrections made during the work

- **Protocol (case 3).** The first run stopped at 50 ms, assuming steady flow
  after about 17 acoustic transits. The flow was still accelerating (mass flux
  147 kg/s), and the case missed at 1.56 % / 1.18 %. The flow is steady by
  150 ms: the worst error is 0.3123 % at 150 ms and 0.3124 % at 300 ms. The
  settle time was lengthened to 200 ms; the 1 % criterion was not changed.

### Recorded finding: the `Reservoir` inlet raises the mass flux

The steady mass flux is 158.6 kg/s against the isentropic 157.24 (+0.9 %),
and it does not fall with the grid. The cause is the existing `Reservoir`
boundary, which takes the incompressible total pressure `p = p₀ − ½ρu²`
(upstream OpenFOAM `totalPressure` without `psi`/`gamma`).

At the inlet Mach of about 0.3, this raises the effective isentropic
stagnation pressure by about 0.13 %. With only a 5 % pressure drop across the
nozzle, the mass flux goes roughly as √Δp, which turns 0.13 % into about 1.3 %.

This is a property of the boundary condition, not of the area terms. A
compressible total-pressure option would remove it, and is not part of this
issue.
