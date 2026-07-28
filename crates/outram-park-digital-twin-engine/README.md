# OUTRAM PARK Digital Twin Engine

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping pass" command). A crate is **complete** only once the maintainer has personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.

**`outram-park-digital-twin-engine`** is the reusable **visualization
framework** for OUTRAM PARK digital twins, plus the offline example simulators
built on it. It provides visual process objects whose rendering derives
directly from physics state — cell count drives displayed cells, temperature
drives cell colour, mass flow drives tracer direction, residence time drives
tracer travel time.

> **Offline demonstrations only.** The example simulators here are teaching and
> capability-building artefacts. They are never connected to live operational
> systems, plant systems, safety-critical infrastructure, or restricted
> infrastructure — see `RESPONSIBLE_USE.md`.

## Design philosophy

Avoid separating physics and rendering unnecessarily. Each visual component
bundles physics state (from `tampines`/`nee_soon`), its visual representation,
and its animation logic together, rather than maintaining a physics model and a
separate rendering model that must be kept in sync by hand.

## What it composes

| Piece | Provided by | Role |
|---|---|---|
| Thermal-hydraulic physics | `tampines` | Component state (temperature, pressure, flow, quality, …) to visualize |
| Reactor-vessel / instrumentation | `nee_soon` | Neutronics/kinetics state to visualize |
| Process control | `chem-eng-real-time-process-control-simulator` | Controller state (setpoints, PID output) to visualize |

## Modules — honest status

| Module | Status |
|---|---|
| `color_maps/` | **Real.** Hot/cold and steam-quality colour functions, ported from the existing validated FHR/CIET simulator examples. |
| `components/` | **Real wrappers.** Ten visual process objects, each composing its `tampines` (or `nee_soon`) physics counterpart. `instrumentation` is a deliberate visual-only label/value placeholder — `nee_soon` exposes no instrumentation-readout type to wrap yet. |
| `animation/` | **Real.** Flow-tracer kinematics: `TracerTrain`, `residence_time_from_flow`, and the `FlowTracer`/`TravelTime` trait contracts. |
| `app_scaffold/` | **Real.** `Arc<Mutex<_>>` physics-thread + panel-dispatch pattern, plus the thread-panic "please restart" modal. |

### Flow tracers

A tracer is a mark travelling along a component's flow path. `TracerTrain`
stores a single phase and derives evenly-spaced marks from it, so the marks
cannot drift apart through floating-point accumulation. Marks advance at
$1/\tau$ of the path per second, where $\tau$ is the component's residence time

$$\tau = \frac{m}{\dot{m}}$$

with $m$ the fluid inventory held in the component and $\dot{m}$ the mass flow
through it. A mark therefore takes exactly one residence time to cross the
component: the animation is a readout of the physical transport time, not a
free-running decorative loop. Doubling the mass flow halves $\tau$ and visibly
doubles the tracer speed. The sign of $\dot{m}$ sets the direction, and zero
flow (unbounded $\tau$) freezes the train rather than inventing motion.

Tracer state is **owned by the application**, not the widget: visual components
are `egui::Widget`s consumed by value and rebuilt every repaint, so a train
owned by a widget would reset its phase to zero each frame.

### Pipes

`PipeVisual` renders a run from either of two state sources, dispatched by the
`PipeVisualState` enum (not a trait object, per the workspace design rules):

- **`Physics(Pipe)`** wraps a full `tampines::components::Pipe`. The per-cell
  temperature profile is read off the flow backend, so the run draws one
  coloured segment per finite-volume cell.
- **`Scalars(PipeScalars)`** takes temperature, mass flow, and residence time
  directly. A `tampines::components::Pipe` needs a `SinglePhaseFluidArray` or
  `CompressibleFluidArray` behind it, which is far more machinery than a short
  connector line between two pieces of equipment. Simulators whose loop physics
  is their own lumped model supply that model's real scalars here. This is a
  *narrower* interface, not a fabricated one.

## Examples

Both examples are **offline demonstration models**, not validated plant models.

```bash
cargo run --release --example htgr_sim_v1
cargo run --release --example fhr_sim_v2
```

### `htgr_sim_v1`

A helium-cooled, graphite-moderated prismatic-block HTGR: reactor kinetics, a
helium primary loop, and a steam secondary loop, drawn entirely on this crate's
reusable widgets.

**What is real:**

- Kinetics wired to `teh-o-prke`'s prompt-excursion layer and
  `DelayedNeutronLayer`.
- Helium `c_p` and density from the CoolProp-derived Helmholtz EOS
  (`outram-park-fork-coolprop`, helium after Ortiz-Vega et al.), re-evaluated
  every step at the current bulk mean temperature.
- Water/steam properties from IAPWS-IF97 (`tampines-steam-tables`) throughout —
  every state is a genuine $(p,h)$ / $(p,s)$ / saturation flash.
- **Two-way loop coupling.** Each step hands the secondary's saturation
  temperature to the primary as the IHX cold-side pinch. The IHX duty follows
  the effectiveness-NTU result for one isothermal side,

  $$Q = \varepsilon \dot{m} c_p (T_{\mathrm{out}} - T_{\mathrm{sink}}), \quad \varepsilon = 1 - e^{-\mathrm{NTU}}$$

  so duty cannot exceed what the temperature difference and $UA$ support. The
  resulting IHX helium outlet becomes the next core inlet, making the core
  inlet a computed loop variable rather than a fixed constant.
- Darcy-Weisbach loop pressure drop with a Haaland friction factor, and the
  circulator power to sustain it.
- A closed steam cycle: condensate is the saturated liquid at condenser
  pressure, the feed pump adds real work $v \Delta p / \eta$, the condenser
  energy balance is carried by a cooling-water stream, and a first-order-lagged
  feedwater controller moves the flow with the duty.

**What is still illustrative:** the *plant data*. Loop geometry, $UA$ values,
efficiencies, inventories, and controller constants are HTGR-scale stand-ins,
not a specific design's numbers. The live steam pressure is still held fixed —
a sliding-pressure or drum model needs a steam-generator mass-and-energy
inventory this single-node model does not carry.

### `fhr_sim_v2`

The fluoride-salt-cooled high-temperature reactor simulator, migrated from
`tampines-steam-tables`.

## Binaries — CIET Educational Simulator v2

Alongside the examples, this crate ships two `[[bin]]` targets for the **CIET
Educational Simulator v2**: a real-time educational simulator of the CIET
integral-effects test facility (UC Berkeley), ported from v1, which remains
available as `cargo run --release --example ciet_educational_simulator` in
`crates/tuas_boussinesq_solver`. Its physics lives in the binary's own module
tree, keeping new physics out of the crate library per this crate's `CLAUDE.md`.

| Binary | What it is |
|---|---|
| `ciet_educational_simulator_v2` | The simulator: TUAS-backed CIET physics, the egui GUI, and an embedded **OPC-UA (IEC 62541)** server on a parallel thread. Also runs `--headless` (physics plus OPC-UA server, no window), which is what makes on-device Termux use possible |
| `ciet_v2_opcua_client` | A small egui **demo OPC-UA client** that finds a running simulator over mDNS, connects, reads the live outputs and writes the controls. Desktop only |

```bash
cargo run --release --bin ciet_educational_simulator_v2
cargo run --release --bin ciet_educational_simulator_v2 -- --headless
cargo run --release --bin ciet_v2_opcua_client
```

The shared interface between them is the `ciet_opcua` **library** module —
`state`, `node_map`, `server`, `discovery`, `pki_paths`. It holds no physics and
no GUI, so it compiles everywhere the workspace targets, including headless for
`aarch64-linux-android`; `async-opcua` uses RustCrypto rather than `openssl-sys`
for exactly that reason. The address space is 36 variables (21 read-only
signals, 8 writable continuous controls, 7 writable switches) at
`opc.tcp://<host>:4840/ciet`, all derived from three enums in
`src/ciet_opcua/node_map.rs`.

**Three things to know before running it:**

- **No authentication, no encryption.** The OPC-UA server uses
  `SecurityPolicy::None` with anonymous access, so anyone on the same network can
  read every value and write every control. That is a deliberate choice for a
  teaching demonstrator; hardening is out of scope. Fine on a phone hotspot, a
  home router, or a lab bench — not on public or untrusted WiFi. Use
  `--bind 127.0.0.1` for local-only.
- **It will not work over campus or enterprise WiFi.** Those networks enable
  client isolation and/or filter mDNS, so both discovery and the direct
  connection fail, and no configuration fixes it. Use a phone hotspot or a home
  router.
- **Offline demonstration only.** Per `RESPONSIBLE_USE.md`, the OPC-UA interface
  exists so standard industrial tooling can drive an offline teaching model on a
  bench. It must never be connected to live operational systems, plant systems,
  safety-critical infrastructure, real-time plant monitoring, or institutional
  production systems.

**Honest status.** The physics is a faithful port of v1's, and the maintainer has
done validation work on **v1** (see
`crates/tuas_boussinesq_solver/verification_and_validation/`), but the **port
equivalence between v1 and v2 has not been verified** — no side-by-side
comparison has been run. The OPC-UA layer has interface unit tests (identifier
uniqueness, clamping, `NaN` rejection, switch round-tripping, browse-name
validity), which verify the contract and say nothing about the physics. Termux
support is **unverified**: the `aarch64-linux-android` `cargo check` is only a
proxy and no native on-device Termux build has been run.

Documentation:

- [`src/bin/ciet_educational_simulator_v2/README.md`](src/bin/ciet_educational_simulator_v2/README.md)
  — running it, the GUI pages, the node tables, security, and limitations.
- [`docs/ciet-v2-opcua.md`](docs/ciet-v2-opcua.md) — the deep OPC-UA reference:
  full node table, write semantics, the discovery design and why there is no
  scanner, the unaddressed threat model, and troubleshooting.

## Android / portability

This crate makes **no Android-portability claim** for its GUI modules — unlike
the rest of the workspace, `egui`/`eframe`/`egui_plot`/`egui_extras` are real
dependencies here, since presentation is the crate's whole purpose. They are
declared under `cfg(not(target_os = "android"))` and the modules that use them
are gated to match, so `cargo check --target aarch64-linux-android` still
passes with the library reduced to `animation/`, which is deliberately
`egui`-free.

## Build & test

```bash
cargo build --release -p outram-park-digital-twin-engine
cargo test --release -p outram-park-digital-twin-engine --lib --tests --examples
```

## Licence

GPL-3.0. See the workspace root for the full licence text and
`RESEARCH_INTEGRITY_AND_PROVENANCE.md` for attribution expectations.
