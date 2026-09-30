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

**Maintainer direction, 2026-09-25, stated as tentative ("perhaps"), and
**settled the same day** by the first model landing on it (below):**

- **Input decks are TOML files**, serialised and deserialised by a
  reader against a **schema**, so a deck is a validated, typed document rather
  than free text. `schema_version = 1`; unknown keys are rejected, not ignored.
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

## Status

Created 2026-09-25 as an **empty skeleton**. ~~Nothing is implemented: there is
no deck format, no visualisation engine, no physics, no public API and no
dependency.~~ **CORRECTED 2026-09-25** — one model has landed, below. There is
still no visualisation engine and no GUI.

## First model: steam methane reforming in a CSTR (2026-09-25)

**Maintainer direction: "draft me a steam methane reforming CSTR in Dover,
headless."** Two independent reactions in a perfectly-mixed tank at steady
state:

$$\text{CH}_4 + \text{H}_2\text{O} \rightleftharpoons \text{CO} + 3\,\text{H}_2 \quad (\text{reforming, strongly endothermic})$$

$$\text{CO} + \text{H}_2\text{O} \rightleftharpoons \text{CO}_2 + \text{H}_2 \quad (\text{water-gas shift, mildly exothermic})$$

The reactor itself is `outram-park-fork-dwsim-libs`' `Cstr` — DOVER supplies the
chemistry and the deck and does not reimplement a reactor.

**What is derived and what is fitted.** Everything thermodynamic is summed from
a five-species formation table: $\Delta H^\circ$, $\Delta S^\circ$, the
equilibrium constants by van 't Hoff, and the reverse rate constants, which are
forced to satisfy $k_f/k_r = K_c$ rather than supplied. The **only** fitted
inputs are the two forward Arrhenius pairs, and they come from the deck. So the
equilibrium limit of this reactor is pure thermodynamics, independent of the
kinetics — which is exactly what
`long_residence_time_approaches_thermodynamic_equilibrium` measures.

### Run it

```bash
cargo run --release -p dover --example smr_cstr -- crates/dover/decks/smr_cstr.toml
cargo run --release -p dover --example smr_cstr -- crates/dover/decks/smr_temperature_sweep.toml
```

Output is CSV on stdout with a stable header and fixed precision; the committed
fixtures in `tests/fixtures/` are regression-checked. There is no GUI and no
window — DOVER is headless by construction here.

### What it produces

Base deck (2 m³, 1123.15 K, 20 bar, steam-to-carbon 3, $\tau = 107$ s):
methane conversion **0.4117**, **1.428** mol H₂ per mol CH₄, duty **76.8 kW**.
Read the conversion as an **overestimate**: the constant-flow assumption below
alone accounts for +9.6 % of it.

### ⚠️ Not validated, and the kinetics are placeholders

The tests are **verification only** — atom balances, thermodynamic consistency,
the equilibrium limit, trend directions. **Not one compares against an
experiment or a published reformer.** Specifically:

- **No published kinetic parameter set is embedded.** The Xu–Froment (1989)
  Langmuir–Hinshelwood constants are the usual choice and are deliberately not
  typed in, because the workspace requires any document informing the code to be
  catalogued in `kovan-literature` first, and that has not been done. The deck's
  forward rates are placeholders and are labelled as such in the deck itself.
- **Isothermal**, with the heat of reaction reported but not fed back into an
  energy balance. Real reforming is violently endothermic.
- **Constant volumetric flow**, inherited from the reactor, while reforming
  takes 2 mol to 4. **Measured 2026-09-25:** this overstates methane
  conversion by **+9.6 %** on the base deck (0.4117 here against 0.3758 from
  compiled upstream DWSIM, which re-flashes the gas every sweep). Handed the
  same `Q` as upstream, the reactor agrees with it to 6.1e-10, so this
  assumption is the entire gap.
- **Power-law kinetics, no adsorption term**, no catalyst, no diffusion, no
  pressure drop, no carbon formation.
- The formation data itself still needs cataloguing in `kovan-literature`.

Do not quote a number out of this as a property of any real reactor.

## Build and test

```bash
cargo build --release -p dover
cargo test  --release -p dover --lib --tests
```

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
