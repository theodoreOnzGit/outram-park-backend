# Code structure

How `outram-park-fork-coolprop`'s source tree is organised, and why. See the
crate root `README.md` for feature status; this file is about *where things
live*, not what's finished.

## Top-level modules (`src/`)

| Module | What it holds |
|---|---|
| `eos.rs` | The Helmholtz-EOS engine: `ResidualTerm`/`IdealTerm` enums (one variant per CoolProp term form), `HelmholtzDerivs`, `FluidEos`. The mathematical core everything else builds on. |
| `fluid.rs` | The `Fluid` enum (one variant per pure fluid) + its `eos()`/`ancillaries()`/`transport()` dispatch. **Fully generated** by `dev/regen_all.py` — do not hand-edit. |
| `fluids/` | One file per pure fluid (`const FluidEos`), **generated** by `dev/gen_fluid.py`. |
| `props.rs` | `(T, ρ)` → full property state (`state_trho`) — the EOS's natural, non-iterative evaluation. |
| `flash.rs` | `(p,T)`/`(p,h)`/`(p,s)` → full property state, via Newton density/temperature solves (the EOS's natural inputs are `(T,ρ)`, not these). |
| `ancillaries.rs` | Fast saturation-pressure/density fits (not iterative), used as VLE initial guesses and standalone lookups. |
| `vle.rs` | The thermodynamically-consistent Maxwell two-phase solve (`T_sat(p)`, `(p,h)` quality) — slower but exact, vs. `ancillaries.rs`'s fast approximate fits. |
| `transport.rs` | Dynamic viscosity `μ` / thermal conductivity `λ` correlations (dilute + higher-order + near-critical enhancement + hardcoded per-fluid forms + Chung corresponding-states). |
| `transport_corresponding_states.rs` | ECS/rhosr-CS scaffolding (not yet implemented — see the crate README). |
| `single_cv.rs` | `OPCPFluidSingleCV` — a `uom`-typed 0-D control volume, the object-oriented entry point most downstream code should use. |
| `openfoam_algorithms/` | Vendored pure-Rust OpenFOAM finite-volume layer + the 1-D compressible `OPCPFluidArray` solver, driven by this crate's EOS. |
| `humid_air/` | ASHRAE RP-1485 humid-air properties (`HAPropsSI` equivalent). |
| `incompressibles/` | The `INCOMP` backend (pure liquids + brines) — 2-D polynomial/exponential property fits, no EOS. `fluids/` (generated) + `fluid_enum.rs` (generated) mirror the pure-fluid layout. |
| `mixtures/` | Multi-fluid Helmholtz + GERG-2008 departure functions. `binary_pairs/` (generated, chunked) mirrors the pure-fluid layout at a larger scale. |

## The three-tier generated-data pattern

Three independent instances of the same pattern, each with its own codegen
pair under `dev/`:

| Data | Enum + dispatch (generated) | Per-item data (generated) | Codegen |
|---|---|---|---|
| Pure fluids | `fluid.rs` | `fluids/<name>.rs` | `gen_fluid.py` / `regen_all.py` |
| Incompressibles | `incompressibles/fluid_enum.rs` | `incompressibles/fluids/<name>.rs` | `gen_incompressible.py` / `regen_incompressible_all.py` |
| Mixture binary pairs | `mixtures/binary_pairs/mod.rs` (hand-written struct + wiring) | `mixtures/binary_pairs/data_<n>.rs` (chunked) | `gen_mixture.py` / `regen_mixture_all.py` |

None of these read JSON at runtime — the `dev/gen_*.py` scripts are
authoring-time tools that read CoolProp's JSON (`upstream_source/CoolProp/`,
gitignored) once and print Rust `const` data. Regenerating is always safe to
re-run; it's how new upstream fluids/pairs get added.

## Tests (`tests/`)

Integration tests are one file per verification topic (not one file per
source module) — e.g. `non_analytic_critical_region.rs`,
`mixture_departure_function.rs`, `all_binary_pairs_smoke.rs`. A "smoke test"
(`all_*_smoke.rs`) checks every generated item evaluates to something
physically sane; a named reference test checks one item against a real
external/derived reference value to tight tolerance. See each test file's
own `//!` doc comment for its specific methodology — and
`verification_and_validation/` for the longer-form write-up of the more
significant ones.
