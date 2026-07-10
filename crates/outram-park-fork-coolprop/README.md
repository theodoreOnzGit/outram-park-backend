<!--
SPDX-License-Identifier: MIT
Part of Outram Park (outram-park-backend).
A fork/translation of CoolProp (https://github.com/CoolProp/CoolProp, MIT).
CoolProp is not affiliated with or endorsing this fork. See NOTICE / TRADEMARKS.md.
-->

# outram-park-fork-coolprop

A pure-Rust fork/translation of **[CoolProp](https://github.com/CoolProp/CoolProp)**
(MIT) — thermophysical properties from Helmholtz-energy-explicit equations of
state — built to OUTRAM PARK's design rules.

This is an **independent fork**, not the CoolProp project and not endorsed by
it (see `TRADEMARKS.md`).

## What's different from CoolProp

- **Enum dispatch, no trait objects.** Fluids are a `Fluid` enum; EOS term
  forms are `ResidualTerm` / `IdealTerm` enums dispatched by `match`. CoolProp's
  string-keyed lookup and backend class hierarchy become exhaustive enums.
- **Hardcoded data, no runtime JSON.** Each fluid's EOS coefficients are `const`
  Rust in `src/fluids/`, generated once from CoolProp's fluid JSON by
  `dev/gen_fluid.py`. The shipped crate reads no files — a few KB per fluid
  (full IAPWS-95 Water is ~3 KB).
- **Pure `std`, no BLAS / C dependencies** — so it also builds for Android.

## Theory

For a pure fluid the reduced Helmholtz energy is
`α(δ, τ) = α⁰(δ, τ) + αʳ(δ, τ)`, with `δ = ρ/ρ_r` and `τ = T_r/T`. All
thermodynamic properties follow from `α` and its first/second `δ`,`τ`
derivatives (Span–Wagner / IAPWS-95). The residual part is a sum of
**Power** (`n·δ^d·τ^t·exp(-δ^l)`), **Gaussian** (bell-shaped) and
**non-analytic** (critical-region) terms; the ideal part is **Lead**,
**LogTau** and **Planck–Einstein** terms.

## Status (2026-07-08)

End-to-end and verified:

- Helmholtz EOS engine covering **every term form CoolProp's 137 fluids use** —
  residual **Power / Gaussian / Exponential / DoubleExponential (Lemmon2005) /
  GaoB** (plus the **NonAnalytic** data, see below) and ideal **Lead / LogTau /
  Planck–Einstein (+ Generalized, FunctionT) / Power / CP0Constant / CP0PolyT /
  CP0AlyLee / EnthalpyEntropyOffset** — with all first/second `δ`,`τ`
  derivatives.
- `(T, ρ)` property evaluation (`props::state_trho`): p, u, h, s, c_v, c_p,
  speed of sound. Plus single-phase `(p,T)`/`(p,h)`/`(p,s)` **flashes**
  (`flash`, density/temperature solves).
- **Saturation & VLE** (`vle`, `ancillaries`): fast saturation-ancillary fits
  (`p_sat`, `ρ'`, `ρ''`; 131 fluids) and a thermodynamically-consistent
  **Maxwell two-phase solve** on the EOS (`T_sat(p)`, `(p,h)` quality). N₂ at
  its normal boiling point → `p_sat` = 101 325 Pa, `ρ'` = 806, `ρ''` = 4.61
  kg/m³; Water at 100 °C → `ρ'` = 958.4 (both matching literature).
- **Transport** (`transport`): dynamic viscosity `μ` (42 fluids) and thermal
  conductivity `λ` (45 fluids) — the CoolProp correlations, the near-critical
  enhancement (**Olchowy–Sengers** + the ammonia/R123 terms), **friction /
  kinetic theory** (methane, H₂S, SF₆, n-pentane, R125), and **all** the
  per-fluid hardcoded formulas: Helium, Water (IAPWS R12-08/R15-11), CO₂
  (Laesecke/Huber), heavy water, the xylenes, R23, hydrogen, benzene, toluene,
  hexane, heptane, ethane, cyclohexane, **methanol** and **methane**. Every
  fluid checked reproduces NIST/IAPWS to ~1–3 % (e.g. water at 25 °C →
  μ=8.90×10⁻⁴ Pa·s, λ=0.607 W/m·K; methane λ=0.0344 W/m·K). The only fluids
  still lacking a viscosity model use the general corresponding-states models
  (Chung, ECS, rhosr-CS) — a separate follow-up; they return `None`.
- **All 137** CoolProp pure fluids are generated as hardcoded Rust
  (`dev/gen_fluid.py` / `dev/regen_all.py`) **and wired into the `Fluid` enum** —
  enumerate them with `Fluid::ALL`; `Fluid::eos/ancillaries/transport`. Data is
  ~0.6 MB (well under the crate's 10 MB budget).
- **`OPCPFluidSingleCV`** — a `uom`-typed 0-D control volume, **two-phase-aware**
  (`(p,h)`/`(p,s)` report vapour quality) with `μ`/`λ` getters.
- **Verification**: CoolProp tabulated states — Water (IAPWS-95) ~1×10⁻⁴ at the
  triple line, Helium machine-precision at Tc, seven fluids' triple-liquid
  `h`/`s` to ≲2×10⁻⁵; a smoke test over **every** fluid at its critical point;
  transport vs NIST and VLE vs literature (`tests/transport_vle.rs`).

Tracked follow-ups (beads `op-kbc`):

- **Non-analytic critical-region terms** — carried in the fluid data but not yet
  evaluated (a no-op, so accuracy within ~1 % of the critical point is
  degraded; unaffected elsewhere).
- **General corresponding-states transport models** (Chung, ECS, rhosr-CS) for
  the ~6 fluids that use them (cyclopentane, isopentane, ethylbenzene, R1234yf,
  R1234ze(E), R152A) — the only remaining viscosity gap.
- **Per-fluid reference tests** beyond the nine already pinned (the rest are
  covered only by the critical-point smoke test).
- **`rfluids` verification** (CoolProp oracle) as a dev-dependency.
- **`uom`-typed public API** (internally raw `f64` SI).

## Regenerating fluid data (codegen)

Fluid EOS data is **hardcoded Rust** in `src/fluids/`, generated once from
CoolProp's fluid JSON by `dev/gen_fluid.py` — the crate never reads JSON at
runtime. The generator is an authoring-time tool; the CoolProp clone it reads
lives in the **gitignored** `reference/` (dev only). All commands below run
**from the crate root** (`crates/outram-park-fork-coolprop/`).

### 0. Get the reference clone

```bash
git clone --depth 1 https://github.com/CoolProp/CoolProp.git reference/CoolProp
```

### 1. One fluid

`gen_fluid.py <FluidName>` prints the Rust for one fluid to stdout; redirect it
to `src/fluids/<module>.rs`:

```bash
python3 dev/gen_fluid.py Water  > src/fluids/water.rs
python3 dev/gen_fluid.py Helium > src/fluids/helium.rs
```

### 2. All fluids (batch)

`dev/regen_all.py` regenerates **every** fluid and rewrites both
`src/fluids/mod.rs` (the `pub mod` declarations) and `src/fluid.rs` (the `Fluid`
enum, its `eos()` dispatch, and `Fluid::ALL`) — so a fresh CoolProp checkout is
wired end-to-end in one command:

```bash
python3 dev/regen_all.py     # -> "regenerated 137 fluids + mod.rs + fluid.rs"
```

Variant names follow the CoolProp fluid name with non-alphanumerics removed and
each token capitalised (`n-Heptane` → `NHeptane`, `R1234ze(E)` → `R1234zeE`,
digit-leading names prefixed with `F`, e.g. `1-Butene` → `F1Butene`);
`Fluid::name` always returns the original CoolProp name.

If CoolProp ever adds an EOS term form this port does not implement,
`regen_all.py` aborts and prints the offending fluid + type name (rather than
emit a wrong number) — that type then needs adding to `src/eos.rs` and
`dev/gen_fluid.py` (see below).

### Current coverage and caveats

The batch generates **all 137** of CoolProp's fluids — every EOS term form they
use is implemented in the engine (`src/eos.rs`). The generator still
deliberately *errors* (rather than emit a wrong number) on any term type it does
not recognise, so if a future CoolProp release adds a new form the batch will
flag it as a `SKIP` with the offending type name.

Supported term forms:

- **Residual (`αr`):** `Power`, `Gaussian`, `NonAnalytic` (data carried; the
  contribution is a documented no-op away from the critical point), `Exponential`,
  `DoubleExponential` (also the `Lemmon2005` R125 form), `GaoB`.
- **Ideal (`α⁰`):** `Lead`, `LogTau`, `PlanckEinstein`, `PlanckEinsteinGeneralized`
  (also `PlanckEinsteinFunctionT`), `Power`, `CP0Constant`, `CP0PolyT`,
  `CP0AlyLee` (lowered to `CP0PolyT` + `PlanckEinsteinGeneralized`),
  `EnthalpyEntropyOffset`.

Two caveats remain:

- **Single-fluid `gen_fluid.py` only writes the `src/fluids/*.rs` file** — it
  does not touch `mod.rs` or the `Fluid` enum. Use `regen_all.py` (above) to
  wire fluids in; all 137 are wired today. A verification test under `tests/`
  is still added by hand (nine fluids are pinned to reference states; the rest
  are covered by the critical-point smoke test).
- **Digit-leading fluid names** (e.g. `1-Butene`) are handled: the `const` name
  is sanitized to a legal identifier (`F_1_BUTENE`), the module file to
  `f_1_butene.rs`, and the enum variant to `F1Butene`.

Adding a *new* term type (should CoolProp introduce one) means implementing its
`α` + first/second `δ`,`τ` derivative contributions in `src/eos.rs` (translating
the matching `IdealHelmholtz*` / `ResidualHelmholtz*` class from CoolProp's
`reference/CoolProp/src/Helmholtz.cpp`) and adding the emit branch in
`dev/gen_fluid.py`.

## References

- CoolProp — <https://github.com/CoolProp/CoolProp> (I. Bell et al., MIT)
- Wagner & Pruß (2002), *The IAPWS Formulation 1995…* (IAPWS-95), J. Phys.
  Chem. Ref. Data 31(2)
- Span & Wagner, multiparameter Helmholtz equations of state
