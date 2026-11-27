# dover

**DOVER** — ***D**eck-based **O**pen-source **V**isualisation **E**ngine for
**R**eactors*.

> ⚠️ **Research, education and V&V only.** Not for nuclear facility operation,
> reactor control, licensing, safety-critical decisions or emergency response.
> See the workspace `RESPONSIBLE_USE.md`.

> ⚠️ **Unverified until validated.** All code in this workspace is
> **unverified and untrusted** unless a specific verification & validation
> (V&V) case demonstrates otherwise.

What DOVER makes **studyable and challengeable** today is one worked case:
a steam-methane-reforming CSTR in which every thermodynamic quantity is
derived from a ten-number formation table, the reverse rates are forced to
satisfy $k_f/k_r = K_c$, and the only fitted inputs (two forward Arrhenius
pairs) sit in a readable TOML deck labelled as placeholders. The places
where the model departs from a real reformer (isothermal, constant
volumetric flow, power-law kinetics) are each stated, and the constant-flow
one is measured against compiled upstream DWSIM.

## Knowledge in this crate

See the workspace [vision](../../VISION.md).

| Knowledge artifact | In this crate |
|---|---|
| Scientific literature | Formation data from the NIST-JANAF Thermochemical Tables (4th ed., Chase 1998) and the CODATA Key Values, as stated in `src/species.rs` (provenance record still owed under `DATA_POLICY.md`). The reactor is `outram-park-fork-dwsim-libs`' `Cstr`, a port of DWSIM's `Reactor_CSTR` (`CSTR.vb`). No published kinetic set (Xu–Froment 1989 is named, not used). |
| Mathematical equations | Reforming and water-gas shift with $\Delta H^\circ$, $\Delta S^\circ$ summed from formation data, $K(T)$ by van 't Hoff, the $K_p$-to-$K_c$ conversion and thermodynamically consistent reverse rates, in `src/smr.rs` (including its "Stiffness" section) and below. |
| Source code | `src/species.rs` (five-species formation table), `src/smr.rs` (reaction set and case), `src/deck.rs` (typed TOML deck, `schema_version = 1`), `src/headless.rs` (CSV runner). |
| Interactive simulations | No GUI. CLI example `examples/smr_cstr.rs` with the decks in `decks/`. |
| Visualisations | None yet. |
| Verification and validation evidence | `tests/smr_cstr.rs`: atom balances, reaction enthalpies against textbook values, the equilibrium limit, trend directions, deck round-trip and rejection, and the committed CSV fixtures in `tests/fixtures/`. Code-to-code against compiled upstream DWSIM, 2026-09-25 (`outram-park-fork-dwsim-libs/tests/upstream_cstr_parity.rs`): given the same `Q`, the `Cstr` agrees with upstream to 6.1e-10 per species; the constant-`Q` assumption overstates methane conversion by +9.6 % (0.411745 vs 0.375803). No validation against experiment. |
| Human reviews and corrections | 0 functions carry a kovan human-review stamp. No entry in `docs/human-corrections-to-ai-work.md`. Struck-through corrections in this README and `AGENTS.md` (the deck reader existing, the retired kovan cataloguing rule); `AGENTS.md` records the $K_p$/$K_c$ slip that moved conversion from 0.197 to 0.412. |

**Status:** the SMR case has verification tests and one code-to-code
comparison of the reactor it uses; nothing is validated, and the kinetics
are placeholders.

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
  (`kovan` uses them for its own files). ~~No crate currently reads a
  *simulation* input deck against a schema, so the deck reader is the new
  part.~~ **CORRECTED 2026-09-30** — that reader now exists: `dover::deck`
  parses a TOML deck into a typed `Deck` (`schema_version = 1`,
  `deny_unknown_fields`), and `dover` depends on `serde` and `toml`.

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
cargo run --release -p deck-based-open-source-visualisation-engine-for-reactors-dover --example smr_cstr -- crates/dover/decks/smr_cstr.toml
cargo run --release -p deck-based-open-source-visualisation-engine-for-reactors-dover --example smr_cstr -- crates/dover/decks/smr_temperature_sweep.toml
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
  typed in, ~~because the workspace requires any document informing the code to be
  catalogued in `kovan-literature` first, and that has not been done~~
  **CORRECTED 2026-11-27** — that cataloguing rule was retired on 2026-10-01
  (this crate's `AGENTS.md`; the maintainer now supplies the corpus), so it no
  longer blocks them; they are still not typed in. The deck's
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
- ~~The formation data itself still needs cataloguing in `kovan-literature`.~~
  **CORRECTED 2026-11-27** — the cataloguing requirement was retired
  2026-10-01 (`AGENTS.md`); what is still owed for the formation data is a
  provenance record under `DATA_POLICY.md`.

Do not quote a number out of this as a property of any real reactor.

## `no_std` + `alloc` (gh:#1643)

The default `std` feature can be turned off: the library is then `no_std` +
`alloc` and builds for bare metal
(`cargo check --release -p deck-based-open-source-visualisation-engine-for-reactors-dover --lib --no-default-features --target x86_64-unknown-none`,
gated by `scripts/check-nostd.sh`). The deck types and their validation, the
SMR physics and the headless CSV runner (`run_deck`, `run_case`) build
without it. What `std` carries: the TOML reader and writer
(`Deck::from_toml`, `Deck::to_toml`, `headless::run_toml`; `toml` 0.8 needs
std, and the serde derives are `cfg_attr(feature = "std", ...)`), the
example and the integration tests.

## Build and test

```bash
cargo build --release -p deck-based-open-source-visualisation-engine-for-reactors-dover
cargo test  --release -p deck-based-open-source-visualisation-engine-for-reactors-dover --lib --tests
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

~~GPL-3.0-only~~ **AGPL-3.0-only** (relicensed 2026-10-20, gh:#973), inherited from the workspace.
