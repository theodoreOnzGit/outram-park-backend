# outram-park-fork-pflotran

An **independent, pure-Rust fork / translation** of
**[PFLOTRAN](https://www.pflotran.org)** — the US-DOE national-lab subsurface
**flow and reactive-transport** simulator — rebuilt to OUTRAM PARK's design
rules: enum dispatch (no trait objects), `uom`-typed API boundaries, a
pure-Rust solver (no PETSc FFI, no MPI in v1), and an Android-buildable library.

> **Independent fork, not the official PFLOTRAN.** This crate is not affiliated
> with, endorsed by, or maintained by the PFLOTRAN development team or the
> national laboratories (LANL, PNNL, ORNL, LBNL, SNL). "PFLOTRAN" is used only
> to identify the upstream work this crate derives from. See `NOTICE` and the
> workspace `TRADEMARKS.md`.
>
> **License: GPL-3.0-only.** PFLOTRAN upstream is LGPL-2.1-or-later; LGPL-2.1
> section 3 lets a licensee relicense a copy under the ordinary GPL, so this
> crate is distributed GPL-3.0-only, consistent with the rest of the suite. The
> exact upstream license must be re-verified byte-for-byte before publish — see
> `NOTICE` and `upstream_source/README.md` (bead op-v6s.1).
>
> **Early v1: RICHARDS solves, but is VERIFICATION-ONLY and has no human V&V.**
> The variably-saturated flow mode runs end-to-end and passes closed-form and
> manufactured-solution verification (2nd-order convergence), but it has **not**
> been validated against published PFLOTRAN reference cases, and no human has
> reviewed it. Use at your own risk. Not for nuclear facility operation, reactor
> control, safety-critical analysis, or licensing decisions — education,
> research, and V&V only.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.

## What exists today

The v1 RICHARDS vertical slice is implemented and tested (verification-only):

| Piece | Module | Status |
|---|---|---|
| Physical-quantity type aliases | `units` | **real** — named `uom` aliases (`FluidPressure`, `Saturation`, `Permeability`, ...) |
| Crate error type | `error` | **real** — `PflotranError` enum; unfinished paths return `NotImplemented`, never a fake result |
| Structured Cartesian FV grid | `grid` | **real** — cells, two-point-flux transmissibility, LDU addressing |
| Fluid EOS + characteristic curves | `properties` | **real** — slightly-compressible water EOS; van Genuchten–Mualem & Brooks–Corey–Burdine curves with analytic (FD-checked) derivatives |
| Input deck + output | `io` | **real (AI-designed subset)** — card-deck parser + CSV/legacy-VTK writers. **Not** real PFLOTRAN deck syntax |
| Newton–Krylov solver | `solver` | **real** — generic Newton driver (static dispatch), Armijo line search, over foam-basic-lib `krylov` (BiCGStab/GMRES + ILU(0)/Jacobi) |
| RICHARDS flow mode | `flow` | **real (verification-only)** — `FlowMode` + `RichardsSimulation`: residual/Jacobian assembly, adaptive timestepping, deck-driven runs; exports a Darcy flow field for transport |
| Conservative solute transport | `transport` | **real (verification-only)** — advection (first-order upwind) + dispersion, implicit Euler, linear direct BiCGStab solve; couples to a RICHARDS flow field |
| TH heat transport | `energy` | **real (verification-only)** — advection–conduction of temperature, implicit Euler; one-way coupled to a RICHARDS flow field (buoyancy feedback deferred) |
| Thermal properties | `properties::thermal` | **real** — water `rho(p,T)`/`mu(T)`/`c_w`/`k_w`, rock thermal properties |
| Aqueous geochemistry | `geochemistry` | **real (verification-only)** — equilibrium speciation (mass-action + mass-balance Newton); ideal activities; minerals/kinetics deferred |

**Test suite:** 84 unit + 5 integration + 1 regression + 3 verification
(MMS 2nd-order convergence, hydrostatic gravity, closed-form advection–diffusion),
all green in release mode; both crates cross-compile to `aarch64-linux-android`.

**Not yet implemented:** GENERAL multiphase (needs the multi-DOF block solver,
op-v6s.4.1 — see `docs/general-multiphase-design.md`); reactive-transport coupling
(transport↔geochemistry); mineral/kinetic geochemistry; two-way (buoyancy) TH
coupling; higher-order (TVD) advection; parallelism (op-v6s.14). All validation
work is deferred (open beads op-v6s.9.x/.10.1/.11.1/.12.1/.13.1).

## Verification results (methodology + measured numbers)

- **MMS spatial convergence** (`tests/verification.rs`): saturated 1D operator
  with a manufactured sinusoidal source; Linf pressure error 40.8 → 10.3 → 2.57
  → 0.64 → 0.16 Pa over 10→160 cells, **observed order 2.000** (2nd-order design
  met). Verification of the two-point-flux discretisation.
- **Closed-form steady state**: gravity-free saturated column reproduces the
  exact linear pressure profile to machine zero.
- These are **verification** (implemented correctly?), not **validation**
  (matches reality?). No comparison to published PFLOTRAN gold-files exists yet
  (bead op-v6s.9). See `docs/ai-directed-decisions.md` for every AI-made choice.

## v1 scope — the vertical slice

The first end-to-end target is deliberately narrow, so a real physics result can
be validated before breadth is added (bead op-v6s.2):

- **Flow mode:** RICHARDS — variably-saturated single-phase groundwater flow.
- **Grid:** structured Cartesian finite volume, two-point flux.
- **Solver:** serial pure-Rust Newton-Krylov (no PETSc, no MPI).
- **I/O:** a minimal card-based ASCII input-deck subset; CSV / VTK output.

Explicitly **out of v1**: unstructured grids, MPI / distributed solves, HDF5,
multiphase (GENERAL) flow, energy transport (TH), solute transport, and
reactive geochemistry (GIRT). Those are later beads (op-v6s.10 .. op-v6s.14).

## Governing equation (RICHARDS, v1)

Liquid-phase mass conservation:

$$ \frac{\partial}{\partial t}\left(\phi\, S_l\, \rho_l\right) + \nabla \cdot \left(\rho_l\, \mathbf{q}_l\right) = Q_l $$

with the Darcy flux

$$ \mathbf{q}_l = -\frac{k\, k_{rl}}{\mu_l}\left(\nabla p_l - \rho_l\, \mathbf{g}\right) $$

where `phi` is porosity, `S_l` liquid saturation, `rho_l` liquid density, `k`
intrinsic permeability, `k_rl` relative permeability, `mu_l` viscosity, `p_l`
liquid pressure, `g` gravity, and `Q_l` a source/sink term. Saturation and
relative permeability follow characteristic (retention) curves of capillary
pressure.

## Roadmap (beads)

Tracked under epic **op-v6s** (`outram-park-fork-pflotran`). Status below is
AI-assessed from the code; bead closure is the maintainer's decision.

- **op-v6s.1** — license + provenance — *done (verify upstream before publish)*
- **op-v6s.2** — scope decision (the v1 slice above) — *done*
- **op-v6s.3** — architecture: enum dispatch, `uom`-typed, no-FFI / no-MPI — *done*
- **op-v6s.4** — pure-Rust Newton-Krylov solver (PETSc replacement, keystone) — *implemented*
- **op-v6s.5** — structured Cartesian finite-volume grid — *implemented*
- **op-v6s.6** — input-deck I/O + gated HDF5 / output — *implemented (HDF5 still deferred)*
- **op-v6s.7** — fluid & material properties (EOS + characteristic curves) — *implemented*
- **op-v6s.8** — RICHARDS flow mode — first end-to-end solve — *implemented (verification-only)*
- **op-v6s.9** — V&V strategy + first RICHARDS benchmark (vs PFLOTRAN gold-files) — *partial: MMS + analytical verification done; validation vs published gold-files still open*
- **op-v6s.10 .. op-v6s.14** — TH, solute transport, reactive geochemistry, GENERAL multiphase, parallelism — *not started*

## Design rules (workspace mandate)

- **Enum dispatch, no trait objects** — flow modes / EOS forms / solver kinds
  are enums matched exhaustively.
- **`uom` at API boundaries** — every physical quantity crossing a public
  boundary is a named `units` alias.
- **Pure Rust, Android-safe** — no PETSc, no MPI, no system BLAS, no C/Fortran
  toolchain in the library build.

## License

GPL-3.0-only. See `LICENSE`, `NOTICE`, and the workspace `TRADEMARKS.md`.
