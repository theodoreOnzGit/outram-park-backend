# CLAUDE.md — `dover` (DOVER)

**DOVER** — ***D**eck-based **O**pen-source **V**isualisation **E**ngine for
**R**eactors*. Backronym set by the maintainer on 2026-09-25 (recorded in
`docs/ecosystem-naming.md`).

Crate-specific guidance. The workspace `CLAUDE.md` at the repository root
governs everything not stated here; this file never relaxes it.

## What this crate is for: the low-fidelity counterpart of DHOBY GHAUT

**Maintainer decision, 2026-09-25: "DOVER is meant to be the low fidelity
equivalent of dhoby ghaut."** `dhoby-ghaut` (**D**igital **H**igh-fidelity
**O**rchestration **by** **G**UI …) hosts the studios that drive the
high-fidelity solvers (`outram-mc`, cfMesh). DOVER is its low-fidelity twin:
the same role, visualising reactors from input decks, over low-fidelity models.

**Still undecided:** which low-fidelity models it drives, the deck schema
(TOML is the current direction, below), and whether it carries a windowing GUI as `dhoby-ghaut` does. Low-fidelity models
already exist in the workspace (e.g. the hierarchical surrogates in
`outram-park-digital-twin-engine`'s simulators), so search before building.
The root `CLAUDE.md` model-hierarchy rule applies to whatever DOVER drives:
hierarchical, physics-derived surrogates, run uncalibrated first.

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

## First model: a steam-methane-reforming CSTR, headless (2026-09-25)

**Maintainer direction, 2026-09-25: "draft me a steam methane reforming CSTR in
Dover, headless."** This is the crate's first physics, and it fixes the deck
direction above from "perhaps TOML" to TOML in fact.

What landed:

- `species` — five-species formation data (CH4, H2O(g), CO, CO2, H2). **The
  NIST-JANAF cataloguing into `kovan-literature` that the workspace hard rule
  requires is OUTSTANDING**: this container has no path to fetch the source.
  Recorded in that module, not glossed over.
- `smr` — two independent reactions (reforming + water-gas shift), `ΔH°`/`ΔS°`
  summed from the species table, `K(T)` by van 't Hoff, reverse rates forced
  to satisfy `k_f/k_r = Kc`. The only fitted inputs are the deck's forward
  Arrhenius pairs.
- `deck` — TOML in, validated and typed, `deny_unknown_fields`, schema
  version 1.
- `headless` — deterministic CSV, stable header, committed fixtures under
  `tests/fixtures/`.

**Reuse, per the search-before-building rule:** the reactor itself is
`outram-park-fork-dwsim-libs`' `Cstr`, not a new one. DOVER supplies the
chemistry and the deck; it does not reimplement a reactor.

### Three findings worth carrying forward

1. **`Kp` is not `Kc`.** `ΔG°` gives `Kp`; `Cstr`'s rate law works in
   concentrations. For reforming `Δn = +2`, so the factor is `(RT/P°)² ≈ 115`
   at 1123 K. Getting this wrong still converges and still returns a
   plausible-looking number — it moved methane conversion from 0.197 to 0.412.
2. **`dwsim-libs` carries two different gas constants.** `reactions::R_GAS` is
   the truncated `8.314` DWSIM upstream uses; the thermo modules use full
   CODATA. At `E = 240 kJ/mol` and 700 K the 5.6e-5 difference in `R` becomes
   2e-3 in `k`. `smr::R_KINETIC` re-exports the one the rate law actually uses.
3. **A fabricated rate constant made the system unsolvable, and it looked like
   a solver bug.** See the "Stiffness" section in `src/smr.rs` for the full
   measurement. Short version: the shift is equilibrium-limited, so its
   pre-exponential does not change the answer (0.41174540 at `1e2` vs
   0.41174800 at `1e6`) but does change whether the residual `ζ − V·rate` is
   solvable at all. Reverting to a conditioned value was the fix; raising
   `max_iter` to 100 000 was not.

### Open against `outram-park-fork-dwsim-libs` (not fixed here)

`Cstr::solve`'s damped Newton, on a step that no damping improves, **takes the
full step anyway** — unbounded, so it can drive a molar flow negative into the
`f.max(0.0)` clamp where the finite-difference Jacobian carries no information.
Upstream bounds its own step to consuming at most 80 % of any compound present
(`CSTR.vb:878-886`); the port did not carry that limit across. A prototype of
the limit was written and measured, and **reverted**: on the badly-posed system
it made convergence *worse*, and changing a mature crate's numerics on the
strength of a case that turned out to be DOVER's own modelling error is not
justified. Reported for the maintainer rather than patched from here.

## Maturity of this model

**Not mature, and not validated.** Every test in `tests/smr_cstr.rs` is
*verification* — atom balances, thermodynamic consistency of the rate law, the
equilibrium limit, trend directions. **Not one compares against an experiment
or a published reformer.** The kinetics are placeholders with no literature
provenance, the model is isothermal, the volumetric flow is held constant while
reforming doubles the mole count, and there is no adsorption term. Do not quote
a number out of it as a property of any real reactor.

## Hard rules for this crate

- **Do not infer the details from the name or the role.** Beyond the SMR CSTR
  above, no further deck format, visualisation engine, physics or API is to be
  invented here until the maintainer decides them. Record each decision and its
  date here and in the README.
- **No placeholder modules, no stubbed API, no TODO physics.** An empty crate
  is honest; a stub that looks like a capability is not.
- **Search the workspace before building anything** (root `CLAUDE.md` hard
  rule). Whatever scope DOVER is given, related code may already exist
  elsewhere in the workspace (for example the egui simulators in
  `outram-park-digital-twin-engine`, or `dhoby-ghaut`, the intended GUI home).
- **Dependencies come from the root `[workspace.dependencies]`** and are added
  only when code here calls into them.
- **Portability.** The library must keep compiling for
  `aarch64-linux-android` (`--all-targets`) and for `wasm32-unknown-unknown`
  (`scripts/check-wasm.sh` picks it up automatically). A future windowing GUI
  dependency must be target-gated off Android in the same change.

## Maturity

**Not declared mature.** See "Maturity of this model" above for what exists and
what it does not establish. Proposing maturity is allowed; declaring it is the
maintainer's call alone.

## API mirror

`docs/dover-api.md` has not been generated yet. Run `kovan-cli api-docs dover`
(not `kovan`, which is the GUI) now that public items exist.
