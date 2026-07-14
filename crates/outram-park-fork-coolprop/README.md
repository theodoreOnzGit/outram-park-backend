<!--
SPDX-License-Identifier: MIT
Part of Outram Park (outram-park-backend).
A fork/translation of CoolProp (https://github.com/CoolProp/CoolProp, MIT).
CoolProp is not affiliated with or endorsing this fork. See NOTICE / TRADEMARKS.md.
-->

# outram-park-fork-coolprop

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


A pure-Rust fork/translation of **[CoolProp](https://github.com/CoolProp/CoolProp)**
(MIT) — thermophysical properties from Helmholtz-energy-explicit equations of
state — built to OUTRAM PARK's design rules.

This is an **independent fork**, not the CoolProp project and not endorsed by
it (see `TRADEMARKS.md`). Ported from
[`CoolProp/CoolProp`](https://github.com/CoolProp/CoolProp), `master` branch,
commit `0e67fe74b30a2fe9526af9bc64ea026a96f56ebf` (2026-07-05) — see
`upstream_source/README.md` for the full provenance record.

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

**Non-analytic critical-region terms are evaluated** (bead op-kbc.6, done
2026-07-10): Water reproduces its defining critical pressure
`p(T_c, ρ_c) = p_c` (22.064 MPa) to `5.2e-14` relative error — see
`tests/non_analytic_critical_region.rs`.

**Humid air** (`humid_air`, ASHRAE RP-1485 / `HAPropsSI`-equivalent, bead
op-kbc.14): `(T,p,W)`/`(T,p,R)` inputs; `W`, `R`, `ψ_w`, specific enthalpy and
volume outputs. Verified against the ASHRAE ideal-gas psychrometric
approximation (`c_p`, `v` agree to <0.1% at 25 °C — see
`tests/humid_air_reference.rs`). Entropy, wet-bulb and dew-point temperature
are not implemented (need CoolProp's ideal-gas reference-state offset
calibration / Brent solves, respectively — see the module doc).

**Chung (1988) corresponding-states viscosity** (`transport::ViscosityModel::Chung`,
bead op-kbc.17, done 2026-07-10): wired for the two fluids CoolProp itself
assigns it to (cyclopentane, isopentane — `dev/gen_fluid.py` detects
`TRANSPORT.viscosity.type == "Chung"`). Verified against the gas-phase
viscosity ballpark for light C5 hydrocarbons (`tests/chung_viscosity.rs`).
**ECS** (ethylbenzene) and **rhosr-CS** (R1234yf, R1234ze(E), R152A) remain
unimplemented — both need a reference-fluid's own transport surface (e.g.
ethylbenzene's ECS maps onto Propane), a materially larger undertaking than
Chung's self-contained correlation; `transport_corresponding_states.rs` is
scaffolded for them.

**Incompressibles** (`incompressibles`, the `INCOMP` backend, bead op-kbc.15,
**done** 2026-07-10): **all 126** CoolProp incompressible fluids, generated by
`dev/gen_incompressible.py`/`dev/regen_incompressible_all.py` (mirroring
`dev/gen_fluid.py`/`dev/regen_all.py` for the pure-fluid side — one file per
fluid under `incompressibles/fluids/`, wired into the `Incompressible` enum in
`incompressibles/fluid_enum.rs`). The 2-D polynomial/exponential/
log-exponential evaluation engine (`T_base`/`x_base` centring, matching
CoolProp `Polynomial2DFrac::evaluate`) is verified against T66 (Therminol 66)
to <1e-6 relative error (`tests/incompressible_t66.rs`) and a smoke test over
all 126 fluids (`tests/all_incompressible_fluids_smoke.rs`). 4 fluids' JSON
carries an all-**zero** never-fit placeholder for one property (e.g.
Acetone's `conductivity`, LiBr's `conductivity`/`viscosity`) — the codegen
detects this and represents it as unavailable (`None`) rather than emit a
knowingly-wrong `0.0`. `polyoffset` (CoolProp's 5th fit form) is unused by any
of the 126 fluids surveyed, so it is not implemented.

**Mixtures** (`mixtures`, multi-fluid Helmholtz + departure functions, bead
op-kbc.16, **done** 2026-07-10): **840 of CoolProp's 888** binary pairs,
generated by `dev/gen_mixture.py`/`dev/regen_mixture_all.py` (mirroring
`dev/gen_fluid.py`/`dev/regen_all.py`; the other 48 touch a component outside
this crate's 137 ported pure fluids — matched by CAS number against the
pure-fluid JSON, since `mixture_binary_pairs.json` uses REFPROP-style short
names, not CoolProp's canonical ones). Binary-pair data lives in
`mixtures/binary_pairs/` (5 generated chunk files, ~400 lines each, to stay
under the crate's file-size convention) plus a hand-maintained `mod.rs`.

The GERG-2008 reducing-function + residual/ideal-gas evaluation engine
implements all three departure-function forms CoolProp's own 28 departure
functions use (`Power`/`Gaussian`/`GergExponential`, each verified against a
finite-difference self-consistency check — `mixtures::departure::self_consistency_tests`)
and is verified against:

- **Nitrogen–Oxygen** (`F = 0`, no departure function) — an air-like 79/21
  mixture reproduces the known speed of sound in air at 300 K to <0.1%
  (`tests/mixture_nitrogen_oxygen.rs`).
- **R32–R125** (`F = 1.0`, a real 8-term departure function — part of the
  R410A refrigerant blend) — the departure contribution is confirmed
  non-negligible against the ideal-mixing-only sum, and the blend state is
  physically sane (`tests/mixture_departure_function.rs`).
- **All 840 pairs** at a dilute supercritical state (`tests/all_binary_pairs_smoke.rs`).

No flash/VLE — only direct `(T, ρ_molar, x)` evaluation, matching what the
original scaffold promised.

### Why the remaining follow-ups are scoped out rather than finished

Incompressibles and mixtures both started as "one hand-transcribed example
proves the engine" gaps (T66; Nitrogen–Oxygen) and were closed the same way:
write a codegen script (`dev/gen_incompressible.py`/`dev/gen_mixture.py`,
mirroring `dev/gen_fluid.py`) and run it over the *entire* CoolProp data set
rather than hand-transcribing more examples. Both codegen passes caught a
real bug neither hand-picked example alone would have surfaced — Acetone's
all-zero conductivity placeholder (incompressibles) and, while deriving the
`xi`/`zeta` → `γ_T`/`γ_v` conversion for six refrigerant-blend pairs that lack
`mixture_binary_pairs.json`'s usual `betaT`/`gammaT` fields (mixtures) — plus a
Python `int`-vs-`float` codegen bug (department-function `d`/`l`/`t` fields
came through as bare integers, rejected by Rust's `f64` fields) caught
immediately by `cargo build`, not by inspection. That's the general lesson:
whenever the plan is "port more of CoolProp's own data," do it via codegen +
a full-coverage smoke test, not incremental hand-transcription.

What's genuinely still scoped out is **a materially different algorithm, not
more of the same** — mixtures has no flash/VLE (only direct `(T,ρ,x)`
evaluation, matching what was originally promised; phase equilibrium for an
*N*-component real-gas mixture is a substantially larger undertaking than the
reducing-function/departure-function engine itself), and:

- **ECS/rhosr-CS transport** (~4 fluids, e.g. ethylbenzene). Chung is
  self-contained (needs only the fluid's own `T_c`/`V_c`/acentric/dipole);
  ECS instead maps a fluid onto a *reference fluid's own transport surface*
  via shape-factor correlations — a second transport subsystem, not an
  extension of Chung.
- **`humid_air` entropy, wet-bulb, dew-point.** Entropy needs CoolProp's
  ideal-gas reference-state offset calibration
  (`ensure_ref_offsets` in `HumidAirProp.cpp`) — evaluating the real
  Water/Air EOS at fixed reference points to pin absolute IAPWS/Lemmon-
  convention constants; the simpler polynomial path used for enthalpy has no
  entropy equivalent (CoolProp's own source has `"Not implemented"` on that
  branch). Wet-bulb/dew-point need a bracketed root-finder (Brent) wrapping
  the whole property evaluation, not just more data.

Three real issues were found this way, none by inspection — all by checking
computed values against known references or running the codegen over the
full fluid set instead of stopping at one hand-picked example:
`humid_air`'s `c_aaw` was missing a `1/rhobarstar²` factor (four orders of
magnitude too large — see `virials.rs`); the incompressible `Exponential`
viscosity form was centred on the wrong base temperature (also four orders of
magnitude off — see `incompressibles/mod.rs`); and once the codegen ran over
all 126 incompressible fluids instead of just T66, Acetone's conductivity
evaluated to exactly `0.0` — not a porting bug this time, but a genuine
CoolProp upstream quirk (an all-zero, never-fit placeholder typed as if it
were a real `polynomial` fit) that the codegen now detects and represents as
unavailable (`None`) rather than silently emit.

Tracked follow-ups (beads `op-kbc`):

- **Per-fluid reference tests** beyond the nine already pinned (the rest are
  covered only by the critical-point smoke test).
- **`rfluids` verification** (CoolProp oracle) as a dev-dependency.
- **`uom`-typed public API** (internally raw `f64` SI).

## Regenerating fluid data (codegen)

Fluid EOS data is **hardcoded Rust** in `src/fluids/`, generated once from
CoolProp's fluid JSON by `dev/gen_fluid.py` — the crate never reads JSON at
runtime. The generator is an authoring-time tool; the CoolProp clone it reads
lives in the **gitignored** `upstream_source/` (dev only). All commands below run
**from the crate root** (`crates/outram-park-fork-coolprop/`).

### 0. Get the upstream_source clone

```bash
git clone --depth 1 https://github.com/CoolProp/CoolProp.git upstream_source/CoolProp
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

- **Residual (`αr`):** `Power`, `Gaussian`, `NonAnalytic` (evaluated — verified
  to `5.2e-14` relative error at Water's exact critical point, bead op-kbc.6),
  `Exponential`, `DoubleExponential` (also the `Lemmon2005` R125 form), `GaoB`.
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
`upstream_source/CoolProp/src/Helmholtz.cpp`) and adding the emit branch in
`dev/gen_fluid.py`.

## Regenerating incompressible-fluid data (separate codegen family)

The `incompressibles` backend (bead op-kbc.15) has its own codegen pair,
mirroring the one above but reading
`upstream_source/CoolProp/dev/incompressible_liquids/json/` instead:

```bash
python3 dev/gen_incompressible.py T66 > src/incompressibles/fluids/t66.rs   # one fluid
python3 dev/regen_incompressible_all.py   # -> "regenerated 126 incompressible fluids + mod.rs + fluid_enum.rs"
```

`regen_incompressible_all.py` rewrites `src/incompressibles/fluids/mod.rs`
(the `pub mod` declarations) and `src/incompressibles/fluid_enum.rs` (the
`Incompressible` enum, its `data()` dispatch, and `Incompressible::ALL`).
Supported fit forms: `polynomial`, `exppolynomial`, `exponential`,
`logexponential` — all four CoolProp's own 126 incompressible-liquid JSON
files use; `polyoffset` (CoolProp's fifth form) is unused by any of them, so
`gen_incompressible.py` aborts with a clear message rather than emit a wrong
number if a future fluid ever needs it. A property whose JSON coefficients are
all zero (never actually fit upstream — e.g. Acetone's `conductivity`) is
detected and generated as `None`, not a knowingly-wrong `0.0`.

## Regenerating mixture binary-pair data (separate codegen family)

The `mixtures` backend (bead op-kbc.16) has its own codegen pair, reading
`upstream_source/CoolProp/dev/mixtures/mixture_binary_pairs.json` +
`mixture_departure_functions.json`:

```bash
python3 dev/gen_mixture.py Nitrogen Oxygen   # one pair, prints a BinaryPair literal
python3 dev/regen_mixture_all.py             # -> "regenerated 840 binary pairs across 5 chunk(s) + mod.rs (48 pairs skipped ...)"
```

Fluid-name resolution is by **CAS number** (every pure-fluid JSON's
`INFO.CAS`), not string matching — `mixture_binary_pairs.json` uses
REFPROP-style short names (`"CYCLOHEX"`) that don't match this crate's
canonical CoolProp names (`Cyclohexane`) directly. A pair is skipped (not an
error) if either component's CAS doesn't resolve to one of this crate's 137
ported pure fluids (48 of CoolProp's 888 pairs, as of this writing — mostly
touching fluids like R1216 or isooctane that aren't in the 137).

`regen_mixture_all.py` writes `src/mixtures/binary_pairs/data_<n>.rs` (~200
pairs per chunk, keeping each file under the crate's file-size convention)
and rewrites the `CHUNKS`/`all()`/`lookup()` wiring in
`src/mixtures/binary_pairs/mod.rs` (which also holds the hand-written
`BinaryPair` struct — only the wiring block is regenerated). Six pairs
(refrigerant blends like R32/R134a) carry `xi`/`zeta` instead of the usual
`betaT`/`gammaT`/`betaV`/`gammaV`; `gen_mixture.py` converts these via
CoolProp's `LemmonAirHFCReducingFunction::convert_to_GERG` formula
(`ReducingFunctions.h`) rather than special-casing them in the Rust engine.

Departure functions (`F ≠ 0`, 40 of the 840 ported pairs) are looked up by
name in `mixture_departure_functions.json` and translated into
[`mixtures::departure::DepartureTerm`](src/mixtures/departure.rs) values —
`Power` for CoolProp's `"Exponential"` type, `GergExponential` for
`"GERG-2008"`, `Gaussian` for `"Gaussian+Exponential"` (ported from
`ResidualHelmholtzGeneralizedExponential::add_{Power,GERG2008Gaussian,Gaussian}`
in CoolProp's `include/CoolProp/fluids/Helmholtz.h`). Each departure type
technically splits into a leading power-only block plus a Gaussian-style
tail (`Npower` in the JSON), but since the power-only block's `η`/`β` are
always `0` in CoolProp's own data — and both `Gaussian` and `GergExponential`
degenerate to a pure power term exactly when `η = β = 0` — every term is
emitted uniformly, with no special-casing needed (see `departure.rs`'s module
doc and `gen_mixture.py`'s `build_departure_terms`).

## References

- CoolProp — <https://github.com/CoolProp/CoolProp> (I. Bell et al., MIT)
- Wagner & Pruß (2002), *The IAPWS Formulation 1995…* (IAPWS-95), J. Phys.
  Chem. Ref. Data 31(2)
- Span & Wagner, multiparameter Helmholtz equations of state
