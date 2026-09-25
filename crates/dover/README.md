# dover

**DOVER** — ***D**eck-based **O**pen-source **V**isualisation **E**ngine for
**R**eactors*.

> ⚠️ **Research, education and V&V only.** Not for nuclear facility operation,
> reactor control, licensing, safety-critical decisions or emergency response.
> See the workspace `RESPONSIBLE_USE.md`.

> ⚠️ **Unverified until validated.** All code in this workspace is
> **unverified and untrusted** unless a specific verification & validation
> (V&V) case demonstrates otherwise.

## What DOVER is for: the low-fidelity counterpart of DHOBY GHAUT

**DOVER is the low-fidelity equivalent of
[DHOBY GHAUT](../dhoby-ghaut/README.md)** (maintainer, 2026-09-25).
DHOBY GHAUT is the **D**igital **H**igh-fidelity **O**rchestration GUI home:
its studios drive the high-fidelity solvers (`outram-mc` Monte Carlo, cfMesh
meshing). DOVER occupies the same role at **low fidelity**, visualising
reactors from input decks.

The details are **not decided yet**: which low-fidelity models it drives, the
deck schema (TOML is the current direction, below), and whether it carries a
windowing GUI as DHOBY GHAUT does. Each needs the maintainer's decision, and
the workspace must be searched before any of it is built. Low-fidelity models already live elsewhere, e.g. the
hierarchical surrogates in `outram-park-digital-twin-engine`'s simulators.


## Direction: TOML input decks, steady-state and dynamic runs (2026-09-25)

**Maintainer direction, 2026-09-25, stated as tentative ("perhaps"):**

- **Input decks are perhaps TOML files**, serialised and deserialised by a
  reader against a **schema**, so a deck is a validated, typed document rather
  than free text.
- DOVER runs **steady-state** simulations, **like DWSIM** (a flowsheet solved
  to steady state), and **dynamic** simulations as well.

**Already in the workspace; reuse before writing anything (checked
2026-09-25):**

- `outram-park-fork-dwsim-libs`: the Rust translation of DWSIM's kernels,
  including `flowsheet`, `flowsheet_solver` and `dynamics` modules plus the
  unit operations (heater, cooler, pump, valve, heat exchanger, separator, …).
  The DWSIM-like steady-state path should compose these, not duplicate them.
- `chem-eng-real-time-process-control-simulator`: exact zero-order-hold
  transfer-function blocks for dynamic process-control runs.
- `serde` and `toml` are already in the root `[workspace.dependencies]`
  (`kovan` uses them for its own files). No crate currently reads a
  *simulation* input deck against a schema, so the deck reader is the new part.

Not decided: the schema's form and versioning, which models a deck may name,
and whether DOVER has a windowing GUI.

## Status: empty skeleton

This crate was created on 2026-09-25 at the maintainer's direction as an
**empty skeleton**. ~~The scope is still to be decided.~~ **CORRECTED
2026-09-25**: the role is set (above), and the details are open. Nothing is
implemented: there is no deck format, no visualisation engine, no physics, no
public API and no dependency. Do not describe this crate as providing anything until code has
actually been written here.

## Build and test

```bash
cargo build --release -p dover
cargo test  --release -p dover --lib --tests
```

The only test asserts that the crate builds and links.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.

## Licence

GPL-3.0-only, inherited from the workspace.
