# outram-park-fork-moltres

MSR neutronics + thermal-hydraulics on the outram-foam finite-volume layer — physics formulation from the LGPL-2.1 [Moltres](https://github.com/arfc/moltres) code, reimplemented on `outram-foam-basic-lib` rather than MOOSE/PETSc.

Circulating-fuel molten-salt reactor multiphysics: multigroup neutron diffusion, **delayed-neutron precursor drift** (the defining MSRE effect — precursors advected out of the core by the flowing fuel salt), and salt thermal-hydraulics, coupled on the OUTRAM PARK finite-volume mesh. Moltres is MOOSE/finite-element; this is an independent finite-volume reimplementation of the same validated formulation (no MOOSE/PETSc/MPI, per the workspace no-FFI rule).

> **⚠️ Scaffold — no human V&V.** Port in progress under the MSRE digital-twin
> epic (`op-6w0`). Independent OUTRAM PARK fork; not affiliated with the
> upstream project. Not for nuclear facility operation, reactor control,
> safety-critical, or licensing decisions.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.
