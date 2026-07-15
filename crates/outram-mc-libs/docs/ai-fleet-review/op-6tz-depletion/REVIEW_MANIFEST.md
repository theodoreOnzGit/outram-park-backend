# REVIEW MANIFEST — depletion / transmutation driver (op-6tz.18)

**⚠️ AI-GENERATED DRAFT — HUMAN REVIEW REQUIRED per `RESPONSIBLE_USE.md`.**
Everything below was produced by AI assistants (Claude Opus 4.8, a lead + two
subagents) and is **untrusted draft material** until a human reviews the physics,
the numerical method, and the data wiring. It passes `cargo build`/`cargo test`
in release, but has **not** been validated against an external benchmark — the
comparison here is against analytic Bateman solutions and the notebook's
*qualitative* trends, not its absolute pin-cell k values.

- **Date:** 2026-07-15
- **Crate:** `outram-mc-libs` (only crate touched). Consumes
  `njoy-outram-park-fork` (cross sections), `openmc-endf-8-depletion-lib-a/-b`
  (decay data), `fission-yields-data` (fission yields) — **no changes made to any
  of those crates** (the njoy fleet op-3ut runs there concurrently).
- **Bead:** op-6tz.18 — *outram-mc: depletion / transmutation driver*.
- **V&V stage:** Prototype → **Unit Tested / Integrated** (verification against
  analytic references; no external-benchmark validation of absolute k yet).

---

## 1. What got built

A new `outram_mc_libs::depletion` module — the transmutation solver + burnup loop
that OpenMC keeps in `openmc/deplete/`. The `depletion` notebook test is now
**LIVE** (was `#[ignore]` + `unimplemented!()`).

| File | Author | Role |
|---|---|---|
| `src/depletion/mod.rs` | lead | Module docs, shared `ReactionRates`/`MicroRate` types, re-exports, unit conventions. |
| `src/depletion/matrix.rs` | lead | `DepletionMatrix` — dense burnup matrix `A` (`dN/dt = A N`, `1/s`), sign/index convention. |
| `src/depletion/cram.rs` | subagent A | **CRAM** order-16 (+48) IPF matrix-exponential solver `exp(A·dt)·N`, port of OpenMC `cram.py`. |
| `src/depletion/chain.rs` | subagent B | **`DepletionChain`** — decay/branching/reactions/yields → `DepletionMatrix`; consumes the data crates. |
| `src/depletion/operator.rs` | lead | **Burnup loop** (`deplete_predictor`) coupling one-group reaction rates → CRAM step; plus `mc_keff_of_actinide_sphere`. |
| `tests/openmc_notebooks/depletion.rs` | lead | LIVE notebook test: inventory + k_inf trends, CSV emit, MC-coupling demo. |
| `Cargo.toml` | lead | Added the three pure-Rust data deps (workspace-inherited). |
| `src/lib.rs`, `tests/openmc_notebooks.rs` | lead | Register module + mark harness row LIVE. |

Architecture mirrors OpenMC's split: `matrix` + `cram` (numerics) ⟂ `chain`
(data) ⟂ `operator` (coupling). Enums for reaction-kind dispatch (no trait
objects); no `Box`, no lifetimes; raw `f64` with documented units per the crate
convention.

---

## 2. Provenance

- **CRAM algorithm + coefficients:** OpenMC `openmc/deplete/cram.py` (MIT),
  `IPFCramSolver.__call__` and the order-16/48 Pusa (2016) coefficient tables
  (M. Pusa, *Nucl. Sci. Eng.* 182:3, 297–318, doi:10.13182/NSE15-26). Ported
  verbatim; cited `file:line` in the doc comments.
- **Chain structure + `simple()`:** OpenMC `openmc/deplete/chain.py`
  (`Chain.form_matrix`) and `examples/pincell_depletion/chain_simple.xml` (MIT) —
  the exact 9-nuclide chain (I135, Xe135, Xe136, Cs135, Gd157, Gd156, U234, U235,
  U238) the notebook uses. Every half-life / yield / Q transcribed from the XML
  and verified against the file.
- **Decay data (live):** `openmc-endf-8-depletion-lib-a/-b` (ENDF/B-8 decay chain
  as packaged by OpenMC).
- **Fission yields (live):** `fission-yields-data` v0.1.4 (ENDF/B-VIII.0
  parent-independent yields, MT=454).
- **Notebook reference:** `depletion.ipynb`
  (openmc-notebooks@`cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT). k values and
  the setup were read from the raw notebook.
- **Cross sections:** `njoy-outram-park-fork` CORE-125 WMP+MGXS via
  `Nuclide::from_core` (all 9 chain nuclides are in the CORE set).

GPL-3.0 OUTRAM PARK translation; not the official OpenMC software; not for
facility operation, licensing, or safety-critical use (RESPONSIBLE_USE.md).

---

## 3. Key assumptions & fidelity scope (HONEST — read before trusting a number)

The transmutation **solver** (CRAM) is exact to machine precision. The **burnup
loop** is a deliberately reduced-fidelity, one-group, infinite-medium
demonstration:

1. **One-group cross sections at a single thermal point (0.0253 eV).** Not a
   transport-tallied multi-group flux spectrum — so no resonance self-shielding,
   no epithermal/fast contribution. Rates are frozen over each 30-day step
   (predictor / forward Euler, matching the notebook's `PredictorIntegrator`).
2. **`k_inf` is a one-group ratio `Σ(N·ν·σ_f)/Σ(N·σ_a)` over the chain nuclides
   only** (O-16 / clad / moderator omitted — a one-group infinite-medium estimate
   ignores moderation anyway). It is a **relative trend indicator**, **NOT
   comparable in absolute value** to the notebook's continuous-energy pin-cell MC
   k (~1.46). Our BOL k_inf ≈ 1.918 is inflated precisely because U-238 resonance
   capture and moderator absorption are absent.
3. **Flux is power-normalised** so fission power in the 0.554 cm³ pin equals
   174 W; magnitude is physical (~1.8e13 n/cm²/s) but spectrum-shape effects are
   not modelled.
4. **Cross sections held constant over the 6-month run** (no per-step re-lookup);
   negligible for this demonstration.
5. **I-135 metastable lumping.** The decay lib splits I-135 β to `Xe135`
   (0.83491) + `Xe135_m1` (0.16509); `chain_simple.xml` and this chain lump both
   into `Xe135` at 1.0, following the notebook.

---

## 4. LIVE vs IGNORED

- **LIVE — `depletion::depletion_burnup`** (integration test). Runs the full
  burnup loop and asserts the 5 physically-required trends the notebook exhibits.
- **LIVE — `depletion::depletion_mc_transport_coupling`**. Feeds the evolved
  actinide inventory to a real MC `run_keff` (bare sphere) — demonstrates the
  depletion→transport path executes; NOT a benchmark (fast bare sphere).
- **LIVE — `src/depletion/cram.rs` tests (6)** — analytic Bateman verification.
- **LIVE — `src/depletion/chain.rs` tests (6)** — chain assembly + live data
  consumption.
- **Nothing left `#[ignore]`d for this bead.** No fake-green: every assertion is
  on a computed result, and the absolute-k gap is documented, not hidden.

---

## 5. Actual build / test output (2026-07-15, release)

```
cargo test -p outram-mc-libs --release --lib depletion
  12 passed; 0 failed   (cram: 6, chain: 6)

cargo test -p outram-mc-libs --release --test openmc_notebooks
  14 passed; 0 failed; 16 ignored   (incl. depletion_burnup, depletion_mc_transport_coupling)

cargo test -p outram-mc-libs --release --lib
  62 passed; 0 failed   (no regressions)

cargo check -p outram-mc-libs --lib --target aarch64-linux-android
  Finished (clean) — the 3 new data deps are pure Rust, Android-safe
```

### Measured numbers

**CRAM analytic verification** (relative error vs analytic Bateman):
- single-nuclide half-life (I-135): 4.0e-16
- A→B→C chain N_C: 2.1e-16; total-atom drift: 0.0 (exact)
- stiff system (λ 1e-2 vs 1e3): 6.0e-16 (gate 1e-5)

**Chain live-data cross-checks:**
- λ(I-135) = ln2/23652 = 2.9306071e-5 1/s; λ(Xe-135) = ln2/32904 = 2.1065742e-5 1/s
- decay-lib half-lives (I-135 = 23652.0 s, Xe-135 = 32904.0 s) match
  `chain_simple.xml` exactly (rel diff 0)
- `u232_thermal_fission_yield(Xe135).value` = 0.00645867 (live consume proof)

**Burnup trajectory** (one-group, notebook UO₂ pin @ 174 W, 6 × 30 d):

| day | U-235 [a/(b·cm)] | Xe-135 | I-135 | k_inf |
|---|---|---|---|---|
| 0 | 9.860e-4 | 0 | 0 | 1.91838 |
| 30 | 9.601e-4 | 4.630e-9 | 9.858e-9 | 1.88166 |
| 90 | 9.083e-4 | 4.453e-9 | 9.844e-9 | 1.87238 |
| 180 | 8.307e-4 | 4.176e-9 | 9.819e-9 | 1.85661 |

- U-235 consumed over the cycle: **1.553e-4 atoms/(b·cm)**, consistent with the
  energy balance `P·t/Q ≈ 1.5e-4` — a real order-of-magnitude conservation check.
- k_inf swing **−6177 pcm** — same **sign and order** as the notebook's MC k
  swing **−4110 pcm** (1.46478 → 1.42368). Absolute values differ by design
  (see §3.2).
- Xe-135 reaches equilibrium inside the first step (~4.6e-9) then holds — the
  xenon-poisoning plateau the notebook shows.
- MC coupling demo (fast bare sphere, 500 hist): BOL k 0.174, EOL k 0.160
  (falls as fissile depletes) — path exercised; absolute value not comparable.

Comparison CSV written to
`verification_and_validation/openmc_notebook_comparisons/depletion.csv`
(gitignored, regenerated by the test).

---

## 6. Additional data / interface needed (reported, NOT worked around)

These are gaps in the **consumed** crates found while wiring; they were **not**
fixed here (no edits to njoy or the data crates). Follow-ups filed in beads.

1. **`openmc-endf-8-depletion-lib-a/-b`: neutron-reaction targets are private.**
   On `SerdeNuclideData` only `name`, `half_life_seconds`,
   `decay_energy_electronvolt`, and `raw_decay_data` are public; the `reaction`
   field (`(n,gamma)` / `(n,2n)` targets and Q) is **private**. So the
   transmutation *targets* cannot be pulled from the decay libs — they are taken
   from the hardcoded `chain_simple.xml` transcription. **Ask:** expose the
   reaction list (target + type + Q) publicly so `DepletionChain` can build the
   neutron-transmutation edges from library data for arbitrary chains.
2. **`fission-yields-data` 0.1.4: per-nuclide thermal accessors are not public.**
   Only `u232_thermal_fission_yield` is re-exported by the prelude; the
   `u235`/`u238`/… per-nuclide thermal accessors live in `pub(crate)` modules, and
   `fission_yield_linear_interpolation` needs a `uom`-0.37 `Energy` this crate
   cannot construct (it is on `uom` 0.38). So the U-234/235/238 yields used by
   `build_matrix` are the `chain_simple.xml`-transcribed values. **Asks:** (a)
   re-export the per-nuclide thermal accessors (or a `by-name` lookup returning a
   raw `f64`); (b) consider aligning `uom` to the workspace's 0.38 to remove the
   version clash.
3. **Fission-product cross sections in the CORE-125 set are LOW-tier WMP/MGXS.**
   Fine for a trend demo; a quantitative Xe-135 worth needs the HIGH tier
   (`net-fetch`) or a dedicated FP data path.

---

## 7. Human-verify list (top asks first)

1. **CRAM correctness** (`cram.rs`) — the IPF sequential sweep, the complex
   Gaussian-elimination solve, and the transcribed order-16/48 coefficients. The
   analytic tests pass at machine precision, but confirm the coefficients match
   `cram.py` digit-for-digit and that the IPF (not partial-fraction) form is what
   you want.
2. **Burnup coupling** (`operator.rs`) — the flux/power normalisation
   (`flux_for_power`), the barn↔cm² factor (`1e-24`), the capture = absorption −
   fission split, and whether the one-group / infinite-medium k_inf scope is an
   acceptable honest partial or should be upgraded to a moderated pin-cell +
   spectrum-averaged multigroup rates before this is cited.
3. **Chain assembly** (`chain.rs`) — the `build_matrix` sign convention and the
   `chain_simple.xml` transcription (esp. the Gd-157 `target="Nothing"` sink and
   the I-135 metastable lumping).
4. **Scope honesty** — that the LIVE assertions (trends, not absolute k) are the
   right claim to make, and the absolute-k gap (§3.2, §6.3) is a follow-up not a
   defect.

---

## 8. Follow-up beads (suggested)

- Upgrade to spectrum-averaged multigroup reaction rates from a transport flux
  tally (transport-coupled, not one-group thermal) → notebook-comparable k.
- Moderated pin-cell geometry for the burnup k (reuse the LIVE `pincell` CSG).
- Data-crate interface asks §6.1 / §6.2 (upstream, outside this workspace).
- HIGH-tier (net-fetch) fission-product XS for quantitative Xe/Gd worth.
