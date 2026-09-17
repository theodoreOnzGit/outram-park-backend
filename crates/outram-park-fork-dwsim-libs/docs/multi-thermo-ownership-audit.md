# Audit: stream / property-package ownership in `outram-park-fork-dwsim-libs`

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

**GitHub issue:** [#127 "MultiThermo: Audit current stream / property-package ownership"](https://github.com/theodoreOnzGit/outram-park-backend/issues/127) — bead `op-27qo`, child of the multi-thermo epic [#126](https://github.com/theodoreOnzGit/outram-park-backend/issues/126) (`op-186o`).
**Audit date:** 2026-09-11.
**Audited port commit:** `61c69546` (`claude/outram-blender-mggk7u`), clean tree, 696 lib tests passing in release.
**Audited upstream:** `/home/user/dwsim-upstream`, `git rev-parse HEAD` = `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` — the commit pinned in this crate's `CLAUDE.md`. Read-only; nothing there was edited.

**This is an audit. Nothing was implemented, ported, or refactored.** One
throwaway measurement harness was compiled to produce the numbers in §4.3 and
then deleted; the exact source is reproduced there so the numbers can be
re-derived.

It is the companion to the port-coverage matrix
([`upstream-port-coverage.md`](./upstream-port-coverage.md)), which is the map;
this document drills into one square of it (rows F1, F2, F4, F5, F28 and T1–T11)
and answers the four questions #127 poses.

## 0. Summary of findings

| # | Finding | Status |
|---|---|---|
| 1 | There are **three** independent thermo-selection types today, not one, and they do not compose. | established, §1 |
| 2 | **Nine of eleven equipment modules import nothing from `crate::thermo` at all** — they take enthalpies as arguments and delegate flashing to caller closures. | established, §1.3 |
| 3 | `PropertyPackageModel`'s entire capability surface is **two methods**, and it serves **one** of `StreamSpec`'s **nine** flash specifications. | established, §2 |
| 4 | The four package variants differ in **no** declared capability — only in numerical output. There is nothing for #130 to match on yet. | established, §2.2 |
| 5 | The first thing a second package collides with is **not** the package — it is the **component basis**, which no type owns. | established, §3 |
| 6 | The enthalpy reference state is carried by **no type**. It is an unstated convention on bare `f64`. Exactly one type stores a `T_ref`, and it is settable per instance. | established, §4 |
| 7 | A 25 K reference-temperature mismatch moves the enthalpy by **≈ 2 207 J/mol**, an order of magnitude larger than the difference between the packages' own physics at a shared datum (**100–276 J/mol**). This is #135's failure mode, measured. | established (measured), §4.3 |
| 8 | `Component` carries **no enthalpy of formation at all**, and its `ig_entropy_formation_25c` field is **never read by any code in the crate**. | established, §4.4 |
| 9 | `PhaseProperties::enthalpy_f` / `entropy_f` (the with-formation slots) are **never written** anywhere. | established, §4.4 |
| 10 | Upstream *does* support multi-package flowsheets, but by a mechanism (a stateful package with an 858-site `CurrentMaterialStream` back-pointer) that this port deliberately did not carry — and upstream reconciles reference states **not at all**. | established, §5 |

**The single most consequential finding is #6/#7.** Everything else is a missing
seam that a compiler error would reveal. A reference-state mismatch produces a
plausible number, and the one piece of cross-stream enthalpy arithmetic the
crate performs today (the mixer, §4.2) would consume it silently.

## 1. Who owns thermo today, per code path

### 1.1 The three selection types

| Type | Where | What it holds | Who constructs it |
|---|---|---|---|
| `PropertyPackageModel` (`thermo/property_package.rs:124-139`) | `thermo`, `separator`, `columns`, `saturation` | Nothing — a 4-variant fieldless enum, `Copy` | the caller, per call |
| `ColumnThermo` (`columns/thermo_bridge.rs:139-147`) | `columns` only | `Vec<Component>` **by value**, a `PropertyPackageModel`, a `ColumnEnthalpyModel`, and a `reference_temperature: f64` | `ColumnThermo::new(components, package)` (`:163`) |
| `FugacityModel` (`thermo/gibbs.rs:167-183`) | `reactors/gibbs_reactor.rs` only | `IdealGas`, or `FrozenLnPhi(Vec<f64>)` — caller-supplied `ln φ`, **frozen** through the minimisation | the caller |

These are three parallel universes. `FugacityModel::FrozenLnPhi` is the only
route by which the Gibbs reactor can see EOS non-ideality, and the coefficients
must be evaluated by the caller from `thermo::cubic_eos` and handed in; nothing
converts a `PropertyPackageModel` into one. `ColumnThermo` wraps a
`PropertyPackageModel` but adds two pieces of state (`enthalpy_model`,
`reference_temperature`) that the package itself has no notion of.

### 1.2 Equipment entry points that take a package

Exhaustive, from a grep for `PropertyPackageModel` across `src/` excluding its
own module and excluding `#[cfg(test)]` bodies:

- **`Separator`** (`separator/mod.rs:262-265`) — the **only** equipment struct
  in the crate that stores a package. It is `#[derive(Copy)]` and its *only*
  field is `pub package: PropertyPackageModel`. Components are **not** stored;
  they arrive per call (`flash_isothermal(&self, components, feed, t, p)`,
  `:298-303`).
- **`ColumnThermo`** (`columns/thermo_bridge.rs:163`) — stores the package
  *and* the components, opposite to `Separator`.
- **`saturation::{bubble,dew}_{pressure,temperature}`**
  (`thermo/saturation.rs:981-1050`) — free functions taking a package as the
  last argument.
- **`flowsheet/import`** (`import/mod.rs:432`, `mapping.rs:295-300`) — parses
  a package out of the XML into `ImportedPropertyPackage::model`. Three DWSIM
  class names map (`Raoult`→`Ideal`, `PengRobinson`, `SRK`); everything else
  becomes an `ImportGap::UnsupportedPropertyPackage`.

That is the complete list. **No flowsheet type has a package field**
(`flowsheet/graph.rs:386-410` — the `Flowsheet` struct is objects, order,
`next_id`, queue, results, `dynamic_mode`, `solved`, `error_message`,
`messages_log`, and nothing else), so the importer's parsed package is handed
back through a side channel the solver never reads. That confirms the finding
recorded when #127 was filed, at this commit.

### 1.3 The nine equipment modules that take no package at all

`grep -rn "use crate::thermo" <module>/` returns **nothing** for `heater`,
`cooler`, `pump`, `compressor`, `expander`, `valve`, `pipe`, `heat_exchanger`,
`mixer` and `splitter`. Only `reactors/gibbs_reactor.rs` imports from `thermo`
(`:75`, `thermo::gibbs`), and `separator` and `columns`.

This is not an omission — it is a deliberate and, in this auditor's view,
*good* pattern. `heater/mod.rs` is a set of free functions over `uom`
quantities: `outlet_enthalpy_heat_added(...)` (`:109`),
`duty_from_outlet_enthalpy(...)` (`:131`), and
`duty_from_outlet_temperature<F>(..., outlet_enthalpy_at: F)` (`:181-186`),
where the PH step is a **generic `Fn` closure supplied by the caller**. Same
shape as `Separator::flash_adiabatic`, which delegates its PH flash to a
caller-supplied closure rather than owning an energy-flash driver.

**Consequence for #126/#130.** Ten of the thirteen equipment models are already
package-agnostic in the strongest sense: they never name a package, so they can
never mismatch one. The multi-package problem is concentrated in three places —
`Separator`, `ColumnThermo`, and the Gibbs reactor — plus the stream/mixer layer
in §3–§4.

**But the same delegation is what defers the reference-state problem rather
than solving it** (§4.1): a heater given `h1` and a closure that computes `h2`
has no way to know the two are on the same datum, and no type in its signature
records one.

## 2. What `PropertyPackageModel` actually guarantees (for #130)

### 2.1 The surface is two methods

`thermo/property_package.rs:94-105`:

```rust
pub trait PropertyPackage {
    fn k_values(&self, components: &[Component], x: &[f64], y: &[f64], t: f64, p: f64) -> Vec<f64>;
    fn flash_pt(/* components, z, t, p */) -> Result<FlashResult, FlashError>;
}
```

That is the whole contract. Per the workspace design rules the trait is a
compile-time contract only; dispatch is the `match` in
`PropertyPackageModel::cubic()` (`:144-151`). There is **no** enthalpy, entropy,
density, fugacity, viscosity, or Cp method on the package.

Everything caloric therefore lives *outside* the package abstraction:

| Quantity | Where it actually lives | How the package participates |
|---|---|---|
| ideal-gas H, S | `thermo/ideal_props.rs` | not at all — free functions on `&[Component]` |
| H, S departure | `thermo::cubic_eos::CubicEos::{enthalpy,entropy}_departure` | via `PropertyPackageModel::cubic()`, which is **private** (`:144`) |
| mixture H, S | `thermo/energy_flash.rs:236`, `:298` | not at all — free generics taking `tp_flash` and `enthalpy_departure` **closures** |
| PH / PS flash | `thermo::energy_flash::{flash_ph, flash_ps}` | not at all |
| phase molar H | `ColumnThermo::{liquid,vapor}_molar_enthalpy` (`thermo_bridge.rs:334`, `:359`) | reads `self.package` only to pick `ColumnEnthalpyModel` |

`thermo_bridge.rs:47-63` says this in the source already, flagging
`liquid_molar_enthalpy` as implemented locally because *"`thermo` exposes the
two halves … but no package-level 'give me the molar enthalpy of this phase'
entry point"*.

### 2.2 The spec surface: nine declared, one served

`StreamSpec` (`flowsheet/streams.rs:221-244`) has **nine** variants —
`TemperatureAndPressure`, `PressureAndEnthalpy`, `PressureAndEntropy`,
`PressureAndVaporFraction`, `TemperatureAndVaporFraction`,
`PressureAndSolidFraction`, `VolumeAndTemperature`, `VolumeAndEnthalpy`,
`VolumeAndEntropy` — the full upstream set. (#126's audit note lists four; that
is a partial quote of the same enum, not a disagreement.)

`PropertyPackageModel` serves exactly **one** of them, `TemperatureAndPressure`,
via `flash_pt`. PH and PS exist in `thermo::energy_flash` as free functions
taking closures; PV/TV exist as `thermo::saturation` free functions taking a
package; solid-fraction and the three volume specs have no implementation at
all.

### 2.3 The four variants have identical *declared* capability

This is the direct answer to #130's "audit `PropertyPackageModel` first to see
what capability surface already exists."

`Ideal`, `PengRobinson`, `PengRobinson1978` and `Srk` implement the same two
methods, accept the same arguments, and return the same types. **No variant
declares a capability another lacks**, and no variant can refuse a request. The
differences that exist are numerical and undeclared:

- `Ideal::k_values` returns the composition-independent Wilson estimate and
  ignores `x` and `y` entirely (`:163-164`).
- `Ideal` has **zero enthalpy departure**, so `H_liquid == H_vapor` and the
  latent heat vanishes. `ColumnThermo::new` works around this by silently
  selecting `ColumnEnthalpyModel::IdealWithLatentHeat` for `Ideal` and
  `EosDeparture` for the cubics (`thermo_bridge.rs:164-167`) — a capability
  decision, made implicitly, in the *column* module rather than in the package.
- `PengRobinson1978` is documented as identical to `PengRobinson` below
  `ω = 0.49` (`property_package.rs:131-136`). §4.3 confirms this empirically
  for a benzene/toluene mixture: the two are **bit-identical** there.
- **`k_ij` is hardwired to zero.** `cubic_eos::BinaryInteraction` exists as a
  struct with no data table (coverage matrix F28), and `k_values` passes `None`
  (`property_package.rs:67-70`). So "which packages need `k_ij`" is not yet a
  discriminator: none of them can receive one through this API.
- The **error** surface is also uniform. `flash_pt` returns
  `Result<_, FlashError>` for every variant, and the cubic path additionally
  *silently* falls back to Wilson K-values whenever `ln_phi` returns `None`
  (`:186-212`). A caller cannot distinguish "PR answered" from "PR failed and
  you got Raoult" from the return type.

**Implication.** There is nothing for #130's matcher to match on today. A
`ThermoRequirement`/capability abstraction over the *current* enum would be
matching a set of four elements that are all mutually substitutable at the type
level. The capability surface has to be *created* before it can be matched —
which is a scoping fact worth having before #130 starts, not a defect.

**Inferred, not established:** the natural first discriminators are the ones
§2.2 exposes — *which `StreamSpec` can this package serve* — and the silent
Wilson fallback, which is arguably the first capability that should become
declared (a package that cannot answer should say so rather than answer
approximately). Neither is prescribed here; #102 already records three design
options for where a flowsheet obtains thermodynamics, and this audit does not
re-argue them.

## 3. Where a second package collides first — the component basis

#127 asks where a second package would collide. The answer is that it collides
one level below the package.

### 3.1 There is no component slate, anywhere

- `Flowsheet` has no compound list (`graph.rs:386-410`).
- Each `MaterialStreamData` carries its **own** `Vec<StreamCompound>`, and
  `StreamCompound` (`streams.rs:375-421`) holds only `name` and `molar_mass`.
- `ColumnThermo` owns a `Vec<Component>` **by value** (`thermo_bridge.rs:141`),
  cloned in at construction.
- `Separator` stores no components at all; they arrive per call.
- **There is no `name -> Component` lookup in the crate.** The only source of
  `Component` values is the **seven** hand-entered presets in
  `thermo::component::reference` plus whatever a caller constructs by hand or
  `petroleum` generates.

  > **Correction, 2026-09-11:** this bullet originally said *eight* presets.
  > `thermo::component::reference` has seven — water, methane, ethane,
  > nitrogen, carbon dioxide, benzene, toluene — counted directly from the
  > source. The count is corrected in place; the finding itself stands as
  > written.
  >
  > **Addendum, 2026-09-11:** the missing lookup is now built, over those same
  > seven presets and no new data — `thermo::registry` (`component_by_name`,
  > `ReferenceCompound`, `ComponentLookupError`) and
  > `flowsheet::component_basis` (`resolve_components`, order-preserving,
  > `&[StreamCompound] -> Vec<Component>`). Coverage is unchanged at seven
  > compounds: the mechanism exists, the data question (§ `DATA_POLICY.md` /
  > ChemSep provenance, #78) is untouched and still open.

Every one of these is indexed **positionally**. `k_values`, `flash_pt`,
`liquid_molar_enthalpy` and the mixer's `compound_w` accumulator all assume
`components[i]` and `x[i]` describe the same species, enforced only by a length
check.

### 3.2 The one place two streams' bases meet — and it *is* guarded

`flowsheet_solver/evaluator.rs:329-336`: the mixer compares each inlet's
compound-name list against the outlet's for exact ordered equality and returns
`SolverError::Other("… different compound lists")` otherwise. That is a real
guard and it is the correct shape. It is, as far as this audit found, the only
such check in the crate.

Note what it guards and what it does not: it guards **species identity and
order**, by name. It does not guard the *interpretation* — two streams with
identical compound names whose enthalpies were produced by different packages
or different reference temperatures pass this check unchanged.

### 3.3 Ranked collision surface for #126

1. **Component basis and ordering** (§3.1) — no owner, positional everywhere.
   A representation bridge (#126's `RepresentationBridge`) has nothing to
   bridge *from* until some type owns a slate.
2. **Enthalpy datum** (§4) — no owner, and the mixer consumes it (§4.2).
3. **K-value vectors** — always constructed per call from
   `(components, x, y, T, P)`, never cached or stored. **No collision here**;
   this is a checked-and-cleared item.
4. **Global-ish state** — searched for and **none found**. There is no
   `static`, `OnceLock`, `lazy_static` or thread-local package or component
   registry in the crate. Ownership is entirely by argument or by value.

Items 3 and 4 coming back clean is a genuinely good result and is why #126's own
note that "this is smaller than it looks" holds up.

## 4. Enthalpy and entropy reference states (for #135)

### 4.1 The reference state is carried by no type

Established:

- `thermo::ideal_props` takes `t_ref` and `p_ref` as **explicit arguments**, by
  deliberate design: *"takes the reference pressure/temperature as explicit
  arguments so the departure reference state is a caller decision, not a buried
  literal"* (`ideal_props.rs:56-62`).
- `thermo::energy_flash::mixture_enthalpy` (`:236-243`) and
  `mixture_entropy` (`:298-306`) likewise take `t_ref` (and `p_ref`) per call.
- `ColumnThermo` is the **only** type in the crate that stores one:
  `reference_temperature: f64`, defaulted to `298.15` in `new` (`:175`) and
  **overridable per instance** via `with_reference_temperature`
  (`thermo_bridge.rs:186-189`).
- `PhaseProperties::enthalpy` is `Option<f64>` in **kJ/kg**, documented
  `"(datum is the property package's)"` (`streams.rs:474-475`). The datum is
  named in a comment and represented nowhere.
- `heater`, `cooler`, `pump`, `compressor`, `expander`, `valve` and
  `heat_exchanger` take enthalpies as `uom` `SpecificEnthalpy` — dimensionally
  checked, datum-blind.

So the crate has **three enthalpy conventions in circulation simultaneously**:
`ColumnThermo` in **J/mol** on a stored `T_ref`; `PhaseProperties` in **kJ/kg**
on an unstated datum; the equipment free functions in `uom` **J/kg** on the
caller's datum. `uom` catches none of this — a datum shift is dimensionally
identical to a correct enthalpy.

### 4.2 The one arithmetic that would consume a mismatched datum

`flowsheet_solver/evaluator.rs:380-409` (mixer):

```rust
if let Some(h) = props.enthalpy {
    if h.is_finite() { enthalpy_flow += w * h; }        // :386-389
}
// ...
props.enthalpy = Some(if total_w != 0.0 { enthalpy_flow / total_w } else { 0.0 });  // :403-407
```

A mass-weighted mean of `props.enthalpy` over the inlets. **This is the whole
of the crate's cross-stream enthalpy arithmetic today**, and it is exactly
#135's stated failure mode: two inlets on different data produce a finite,
plausible mixed enthalpy and a wrong energy balance, with nothing to catch it.
It is guarded for compound-list equality (§3.2) and for `NaN` (upstream's own
`Mixer.vb:145` guard) and for nothing else.

The mixer is reachable today — it is one of only four `ObjectType`s the
built-in evaluator handles (`evaluator.rs:204-207`).

### 4.3 Measured: the size of a datum error versus the size of the physics

The claim "a datum mismatch is larger than the model difference it hides" is
the load-bearing one in #135, so it was measured rather than asserted.

Method: `ColumnThermo` over `reference::{benzene, toluene}`, `x = (0.5, 0.5)`,
`T = 350 K`, `P = 101 325 Pa`, release profile, commit `61c69546`. Reproduce by
placing this in `tests/` and running
`cargo test --release --test <name> -- --nocapture`:

```rust
use outram_park_fork_dwsim_libs::columns::thermo_bridge::ColumnThermo;
use outram_park_fork_dwsim_libs::thermo::component::reference;
use outram_park_fork_dwsim_libs::thermo::property_package::PropertyPackageModel;

#[test]
fn audit127_enthalpy_scale_across_packages() {
    let comps = vec![reference::benzene(), reference::toluene()];
    let x = [0.5, 0.5];
    let (t, p) = (350.0, 101_325.0);
    for pkg in [PropertyPackageModel::Ideal, PropertyPackageModel::PengRobinson,
                PropertyPackageModel::PengRobinson1978, PropertyPackageModel::Srk] {
        let th = ColumnThermo::new(comps.clone(), pkg);
        let (hl, hv) = (th.liquid_molar_enthalpy(&x, t, p), th.vapor_molar_enthalpy(&x, t, p));
        println!("{pkg:?}: H_L = {hl:.4}  H_V = {hv:.4}  latent = {:.4}", hv - hl);
    }
    let a = ColumnThermo::new(comps.clone(), PropertyPackageModel::PengRobinson);
    let b = ColumnThermo::new(comps.clone(), PropertyPackageModel::PengRobinson)
        .with_reference_temperature(273.15);
    println!("offset = {:.4} J/mol",
        b.liquid_molar_enthalpy(&x, t, p) - a.liquid_molar_enthalpy(&x, t, p));
}
```

**Results (2026-09-11, commit `61c69546`), J/mol:**

| package | `H_L` | `H_V` | `H_V − H_L` |
|---|---|---|---|
| `Ideal` | −27 737.2331 | 5 204.4415 | 32 941.6745 |
| `PengRobinson` | −27 637.1437 | 4 928.8727 | 32 566.0164 |
| `PengRobinson1978` | −27 637.1437 | 4 928.8727 | 32 566.0164 |
| `Srk` | −28 258.3704 | 4 928.5639 | 33 186.9342 |

Same package (`PengRobinson`), reference temperature moved 298.15 K → 273.15 K:
`H_L` goes −27 637.1437 → −25 429.7329, an offset of **+2 207.4109 J/mol**.

**Interpretation.**

1. **The four packages share one datum today.** `Ideal`'s `H_V` is the pure
   ideal-gas value (its departure is identically zero), and PR's differs from it
   by 275.57 J/mol — the PR vapour departure, which is real physics, not an
   offset. All four run through the same `mixture_ideal_gas_enthalpy` with the
   same `T_ref`. Within the crate as it stands, package-to-package enthalpies
   *are* comparable — provided `T_ref` matches.
2. **`PengRobinson` and `PengRobinson1978` are bit-identical here**, confirming
   the `ω < 0.49` documentation for this mixture. (Benzene ω = 0.210, toluene
   ω ≈ 0.264 — both well under.)
3. **The datum error dwarfs the physics.** 2 207 J/mol from a 25 K reference
   shift, against 100 J/mol (`Ideal` vs `PR` liquid) to 276 J/mol (vapour) of
   genuine model difference at a shared datum — a factor of **8 to 22**. A
   flowsheet that mixed the two would not merely be slightly wrong; the datum
   artifact would be the dominant term, and it would be a constant offset that
   converges perfectly happily.
4. `with_reference_temperature` is public, `#[must_use]`, and documented for a
   legitimate purpose. **Nothing prevents two `ColumnThermo` instances in one
   flowsheet from disagreeing**, and nothing would report it.

**Scope of this measurement.** It is a harness measurement of the crate's own
arithmetic, **not** physics validation. It says what the code computes, not
whether those enthalpies are right.

### 4.4 Formation enthalpy and entropy: absent, and partly dead

- **`Component` has no enthalpy of formation field at all**
  (`thermo/component.rs:42-71`). It has `ig_entropy_formation_25c` (`:70`) and
  nothing for `H_f`.
- **`ig_entropy_formation_25c` is never read by any code in the crate.** A grep
  for `.ig_entropy_formation_25c` across `src/` and `tests/` returns **zero read
  sites**; the only occurrences are the struct field, the `Component::new`
  parameter, the presets, `f64::NAN` in the two `petroleum` constructors
  (`riazi.rs:558`, `pseudo_component.rs:488`) and doc text.
- `ideal_props.rs:100-105` states the position explicitly: the module returns
  *sensible* H and S relative to `T_ref`/`P_ref`, and *"absolute enthalpy/entropy
  of formation … are a separate additive offset, not applied here."*
- **`PhaseProperties::enthalpy_f` and `entropy_f`** — the with-formation slots
  mirrored from DWSIM (`streams.rs:476-481`) — are **never written**: a grep for
  `enthalpy_f =` / `entropy_f =` across `src/` and `tests/` returns nothing.

**Consequence, and a sharpening of #176.** The stream-side blocker recorded in
`tests/cut_to_stream.rs` is exact as written — `validate()` (`streams.rs:1221-1240`)
requires a *finite* mixture `temperature`, `pressure`, `enthalpy` **and**
`entropy`, in that order, and nothing writes the last two because F2's driver
does not exist. But the *data* half of that gap should be attributed precisely:
since `ig_entropy_formation_25c` is read by nothing, the NaN in it is not what
would break a pseudo-component entropy. **The binding data gap is the zeroed
`cp_ig_a..e` coefficients**, which `mixture_ideal_gas_entropy` *would* consume,
returning a finite number with no physical content — which is exactly why
`cut_to_stream.rs` declines to write one. The NaN formation entropy is a
second, currently-inert gap that becomes live only if formation offsets are
ever implemented. Worth correcting on #176 before it is scoped.

**Reaction enthalpy conventions** (#135 also asks for these) were **not
audited** — `reactions.rs` and the `reactors/` energy-balance modes were not
read for this question. Coverage-matrix rows R7/R8 already record reactor
energy-balance modes and thermo coupling as `MISSING + REQUIRED`, so there is
probably little to find, but this audit does not claim that. Flagged rather
than guessed.

## 5. Upstream: what DWSIM actually does (read first, per the workspace rule)

### 5.1 Upstream's ownership model is a stateful back-pointer

- `MaterialStream.vb:282-292` — `GetPropertyPackageObject`,
  `GetPropertyPackageObjectCopy` (returns `PropertyPackage.Clone()`),
  `SetPropertyPackage(pp)`.
- `MaterialStream.vb:994-998` — `AssignSelfToPP()` is one line:
  `PropertyPackage.CurrentMaterialStream = Me`.
- `PropertyPackage.vb:661-679` — the `CurrentMaterialStream` setter is not a
  plain assignment. It **accumulates into a `CompoundPropCache`** keyed by
  compound ID for every compound of every stream ever assigned, then calls
  `RunPostMaterialStreamSetRoutine()` and `CheckCompounds()`.
- **`CurrentMaterialStream` appears 858 times in `PropertyPackage.vb`.** The
  package reads T, P and compositions off the currently-bound stream rather
  than taking them as arguments.

So upstream's package is a **mutable object rebound to each stream in turn**,
and callers must remember to rebind it — which is why `AssignSelfToPP()` is
sprinkled through `Vessel.vb` (`:387`, `:400`, `:405`, `:410`) and `Pipe.vb`
(`:275`, `:735`, `:811`, `:816`, `:831`) at every point a stream is touched.

This port's argument-passing design is **strictly better for #126's purposes**,
and the decision recorded at `flowsheet/streams.rs:97-100` was the right one.
An audit that recommended porting the back-pointer would be recommending a
regression.

### 5.2 Upstream *does* support multi-package flowsheets

`DWSIM.Interfaces/IFlowsheet.vb:60` —
`Property PropertyPackages As Dictionary(Of String, IPropertyPackage)`, with
`AvailablePropertyPackages` alongside (`:62`) and the implementation at
`DWSIM.FlowsheetBase/FlowsheetBase.vb:376`. Each `MaterialStream` holds its own
package reference. So the mechanism is **one dictionary per flowsheet, one
package reference per stream, keyed by id** — and this crate's importer already
parses exactly that structure (`import/mod.rs:481-482`: `property_packages:
Vec<ImportedPropertyPackage>` plus a stream-id → package-id map). The parse side
of #126 is done; only the consumption side is missing.

### 5.3 Upstream reconciles reference states: not at all

This is the part worth knowing before #135 is scoped, and it is a caution, not
a template.

- The cubic and activity packages build enthalpy as
  `RET_Hid(298.15, T, …) + departure` — a **hardcoded 298.15 K** literal
  (`PropertyPackage.vb:10746-10750`, and the same literal throughout the
  package files; `LeeKeslerPlocker.vb` alone has 12 `RET_Hid` call sites).
- `SteamTables.vb:899-969`'s `DW_CalcEnthalpy` returns the **raw IAPWS-IF97
  enthalpy** (`m_iapws97.enthalpySatLiqTW(T)` / `enthalpyW(T, P/1e5)`), which is
  on the IAPWS datum, not the 298.15 K ideal-gas datum.
- `SteamTables.vb:970-972` then forms a "departure" as
  `enthalpyW(T, P/1e5) - RET_Hid(298.15, T, Vx)` — **subtracting two quantities
  on different data**, so the result carries the constant offset between them.

**Established:** upstream mixes an IAPWS-datum enthalpy and a 298.15 K
ideal-gas-datum enthalpy in one expression, and there is no reference-state
translation layer anywhere in `PropertyPackage.vb`.

**Inferred, not established:** that this is harmless when the steam package is
used alone (a constant offset cancels in every enthalpy *difference*, which is
what duties and energy balances are) and exactly the thing that does **not**
cancel when two packages meet at a stream boundary. This audit did not trace the
consumers of `DW_CalcEnthalpyDeparture` in the steam package to confirm the
cancellation, and does not claim it. It is flagged because #135 must not adopt
upstream's convention by imitation.

**The #128 precedent applies here.** That audit established that upstream's
`ZtoMinG` minimum-Gibbs root selection is dead code in all three copies. The
lesson — an upstream mechanism existing is not evidence it works — is why §5.3
is stated as "upstream does not reconcile" rather than "upstream reconciles like
X, copy it."

## 6. Answers to #127's four "to determine" items

**"Which quantities should be canonical and which derived/cacheable."**
The audit does not prescribe, but it narrows the question with one established
fact: today, **nothing is canonical**, because nothing computes anything.
`MaterialStreamData` holds compounds (name + molar mass), an overall
composition, per-phase `PhaseProperties`, a `StreamSpec`, a `FlowSpec` and a
`last_solution_input` snapshot — and `apply_vle_flash` (`streams.rs:1027`)
writes phase compositions and molar flows but **no enthalpy or entropy**
(verified: no `enthalpy`/`entropy` assignment in its body). The existing
`MaterialStreamInputData`-as-derived-cache framing that #126 cites is sound and
is already what the code does. **The physical inventory (compound identities,
their molar masses, the composition, one flow quantity per `FlowSpec`) is the
only thing in the struct that is not already derived**, which matches #126's
governing principle exactly and suggests the split needs no invention.

**"Whether `StreamSpec` should be reused as the bridge specification."**
Established facts bearing on it: `StreamSpec` has nine variants covering the
full upstream set; the three #126 names as bridges (TP, PH, PS) are variants
one, two and three; the package serves only the first, and PH/PS exist as free
functions elsewhere. Reusing it would mean a bridge type whose domain is 3/9
inhabited — expressible, but the type would not say which three are supported.
**This is a design decision for the maintainer, not an audit finding**, and it
interacts with the naming collision #126 already flags (`columns/thermo_bridge.rs`
is a different concept and is well-documented as such).

**"Where units currently obtain their package, and whether that is uniform."**
**It is not uniform, and §1 gives the enumeration.** Three selection types;
`Separator` stores the package but not the components, `ColumnThermo` stores
both, the Gibbs reactor uses a different enum entirely, and nine of eleven
equipment modules take no package at all and receive enthalpies as arguments.

**"What `PropertyPackageModel` actually guarantees as a capability surface."**
Two methods; one of nine flash specs; no caloric properties; `k_ij` structurally
unreachable; a silent Wilson fallback that is indistinguishable from success;
and **no capability difference between the four variants** (§2).

## 7. What this audit did not check

Stated plainly rather than left to inference:

- **Reaction-enthalpy conventions** (§4.4) — `reactions.rs` and the reactor
  energy-balance modes were not read.
- **Whether upstream's steam-package datum mixing is benign** (§5.3) — the
  consumers of `DW_CalcEnthalpyDeparture` were not traced.
- **`dynamics/`** — not examined for thermo ownership. It has no controller
  model (coverage matrix F16) and is unlikely to hold one, but that is inference.
- **Any physics correctness claim.** §4.3 measures what the code computes. It
  validates nothing.
- **`tests/` beyond `cut_to_stream.rs`** was not swept for further ownership
  assumptions.
