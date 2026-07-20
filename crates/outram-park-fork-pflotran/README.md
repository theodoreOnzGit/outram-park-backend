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
> **Scaffold — no flow mode solves yet, and no human V&V.** Use at your own
> risk. Not for nuclear facility operation, reactor control, safety-critical
> analysis, or licensing decisions — education, research, and V&V only.

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

This crate is at the **scaffold** stage. Present and compiling:

| Piece | Module | Status |
|---|---|---|
| Physical-quantity type aliases | `units` | **real** — named `uom` aliases (`FluidPressure`, `Saturation`, `Permeability`, ...) |
| Crate error type | `error` | **real** — `PflotranError` enum; unfinished paths return `NotImplemented`, never a fake result |
| Flow-mode dispatch shape | `flow` | **scaffold** — enum-dispatch `FlowMode` with a `Richards` variant; the solve is not implemented |

Everything else named below is planned, not written.

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

Tracked under epic **op-v6s** (`outram-park-fork-pflotran`):

- **op-v6s.1** — license + provenance (this scaffold; verify upstream before publish)
- **op-v6s.2** — scope decision (the v1 slice above)
- **op-v6s.3** — architecture: enum dispatch, `uom`-typed, no-FFI / no-MPI
- **op-v6s.4** — pure-Rust Newton-Krylov solver (PETSc replacement) — *keystone*
- **op-v6s.5** — structured Cartesian finite-volume grid
- **op-v6s.6** — input-deck I/O + gated HDF5 / output
- **op-v6s.7** — fluid & material properties (EOS + characteristic curves)
- **op-v6s.8** — RICHARDS flow mode — first end-to-end solve
- **op-v6s.9** — V&V strategy + first RICHARDS benchmark (vs PFLOTRAN gold-files)
- **op-v6s.10 .. op-v6s.14** — TH, solute transport, reactive geochemistry, GENERAL multiphase, parallelism

## Design rules (workspace mandate)

- **Enum dispatch, no trait objects** — flow modes / EOS forms / solver kinds
  are enums matched exhaustively.
- **`uom` at API boundaries** — every physical quantity crossing a public
  boundary is a named `units` alias.
- **Pure Rust, Android-safe** — no PETSc, no MPI, no system BLAS, no C/Fortran
  toolchain in the library build.

## License

GPL-3.0-only. See `LICENSE`, `NOTICE`, and the workspace `TRADEMARKS.md`.
