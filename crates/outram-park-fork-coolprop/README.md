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
  speed of sound.
- The codegen (`dev/gen_fluid.py`) turns **all 137** CoolProp fluid JSONs into
  hardcoded Rust; nine are wired into the `Fluid` enum today.
- **Verification** against CoolProp's tabulated states: Water (IAPWS-95) to
  ~1×10⁻⁴ at the triple line (`tests/water_reference.rs`); Helium to machine
  precision at the critical point (`tests/helium_reference.rs`); and the newer
  term forms pinned by seven fluids — Nitrogen, Fluorine, Methanol, R125,
  Ammonia, R22, n-Heptane — reproducing triple-liquid `h`/`s` to ≲2×10⁻⁵
  (`tests/term_types_reference.rs`).

Tracked follow-ups (beads `op-kbc`):

- **Non-analytic critical-region terms** — carried in the fluid data but not yet
  evaluated (a no-op, so accuracy within ~1 % of the critical point is
  degraded; unaffected elsewhere).
- `(T, p)` / `(p, h)` … **flashes** (need a density solve).
- **Wiring the remaining generated fluids** into the `Fluid` enum.
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

Loop over every fluid JSON, deriving a valid Rust module name from each fluid
name and reporting which succeed vs. skip:

```bash
for f in reference/CoolProp/dev/fluids/*.json; do
  name=$(basename "$f" .json)
  # module/file name: lowercase, non-alphanumerics -> '_', digit-leading -> 'f_'
  mod=$(printf '%s' "$name" | tr 'A-Z' 'a-z' | sed 's/[^a-z0-9]/_/g')
  case "$mod" in [0-9]*) mod="f_$mod";; esac
  if python3 dev/gen_fluid.py "$name" > "src/fluids/$mod.rs" 2>/tmp/gen_err; then
    echo "OK    $name -> src/fluids/$mod.rs"
  else
    echo "SKIP  $name : $(cat /tmp/gen_err)"
    rm -f "src/fluids/$mod.rs"
  fi
done
```

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

- **Generating a file is not the same as wiring it in.** Each new fluid also
  needs a `pub mod <module>;` line in `src/fluids/mod.rs`, a `Fluid::<Name>`
  enum variant + `match` arm in `src/fluid.rs`, and (ideally) a reference test
  under `tests/`. The batch loop only writes the `src/fluids/*.rs` files. Nine
  fluids are wired in today (Water, Helium, plus Nitrogen, Fluorine, Methanol,
  R125, Ammonia, R22, n-Heptane — the seven that pin the newer term types in
  `tests/term_types_reference.rs`).
- **Digit-leading fluid names** (e.g. `1-Butene`) are handled: the generator's
  `const` name is sanitized to a legal identifier (`F_1_BUTENE`), and the batch
  loop names the module file `f_1_butene.rs`.

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
