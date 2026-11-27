# OUTRAM PARK Digital Twin Engine

The digital-twin engine of **OUTRAM PARK** (**O**pen-source **U**nified
**TRA**nsient **M**ulti-**P**hysics **A**dvanced **R**eactor simulation
**K**it). It has no station backronym of its own.

> ⚠️ **Research, education and V&V only.** Not for nuclear facility operation,
> reactor control, licensing, safety-critical decisions or emergency response.
> See the workspace `RESPONSIBLE_USE.md`.

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

It answers **"what does a reactor's physics state look like as it evolves,
and can a reader watch it, question it and run it again?"**

What this crate makes **studyable and challengeable** is the step from a
published reactor description to a running, drawn plant: the MSRE and EBR-II
benchmark reports turned into stepping models whose every constant is cited
(and whose disagreements with the measurements are recorded, not fitted
away), HTR-10's cited design data and packed-bed correlations each pinned to
its published value, and animations whose speed is a physical residence time
rather than decoration. A reader can open a simulator in a browser, change a
pump or a rod, and then read the module doc that says which number came from
which report and which choice was uncited.

## Knowledge in this crate

See the workspace [vision](../../VISION.md).

| Knowledge artifact | In this crate |
|---|---|
| Scientific literature | MSRE: ORNL-TM-0728, ORNL-TM-1070, ORNL-TM-1626, ORNL-TM-1647, ORNL-4233 (cited in `src/msre/` and [`docs/msre_library.md`](docs/msre_library.md)). EBR-II: ANL-ARC-226/228 and IAEA-TECDOC-1819, SHRT-17 / SHRT-45R (cited in `src/ebr2/` and [`docs/ebr2_library.md`](docs/ebr2_library.md); AI transcriptions, unreviewed). HTR-10: IAEA-TECDOC-1382 Chapter 4 (core-physics benchmark), Gao and Shi (2002), and the Virtual Test Bed's KTA worked example and ZBS tabulation (`src/htr10/` module doc; more in `crates/kovan-literature/CATALOGUE.md`); `htgr_sim_v1`'s beta_eff from Chen et al. (2009). HTR-10 passive decay heat against Hu et al.'s 206 kW. CIET (UC Berkeley) for the v2 simulator. Not a port of one upstream code. |
| Mathematical equations | Circulating-fuel point kinetics (TM-1626 importance-weighted line model by default, TM-1070 well-stirred as the ablation) with fuel and graphite feedback (`src/msre/`); point kinetics with fast-reactor feedback and lumped fuel / core / IHX / pool (`src/ebr2/model.rs`); KTA packed-bed pressure drop and ZBS bed conductivity (`src/htr10/`); the HTGR plant's energy chain, steam generator and loops in `examples/htgr_sim_v1/physics/` module docs; tracer residence time $\tau = m / \dot{m}$ (below); the twin's reactivity coefficient definition (`src/twin`). |
| Source code | Library: `msre/`, `ebr2/`, `htr10/`, `twin` (multifidelity twin template and its shell), `multifidelity/` (double-pipe HF to LF), `components/` (physics-backed widgets), `headless/`, `opcua_core/`, `ciet_opcua/`, `htgr_opcua/`; re-exports of `bugis`'s `animation`, `color_maps`, `app_scaffold`, `ascii`. Simulators: `examples/` and `src/bin/`. |
| Interactive simulations | egui apps: `htgr_sim_v1`, `fhr_sim_v2`, `distillation_sim_v1`, `widget_studio`, `twin_app`, and the `ciet_educational_simulator_v2` / `ciet_v2_opcua_client` binaries. Browser demos (wasm, built from `web/`): `ciet_web`, `msre_web`, `ebr2_web`, `htgr_sim_v1` (`web/htgr/`), `fukushima_web` (password-gated), `double_pipe_twin_web`. Every egui simulator has a headless mode. CLI: `double_pipe_lf_grid`, `hf_lf_double_pipe_dataset`. |
| Visualisations | The live simulators themselves, plus committed screenshots: `docs/htgr_sim_v1_schematic_tabs/`, `docs/htgr_screenshots/`, `docs/ebr2_web_screenshots/`, `docs/twin_app_screenshots/`, `web/msre/screenshots/`, and the ORNL-4233 pump-trip comparison plots in `docs/msre_pump_trip/`. |
| Verification and validation evidence | MSRE ([`docs/msre_library.md`](docs/msre_library.md), 2026-11-11): with the default line model the circulation loss is +8.1 % against TM-1626's measurement and TM-1647's natural period is +5.0 % at 7.3 MW and +2.0 % at 1.0 MW. EBR-II ([`docs/ebr2_library.md`](docs/ebr2_library.md), 2026-11-10, uncalibrated): SHRT-45R base peak P/F 3.00 against the measured 2.8 (+7 %), spanning 2.14–3.35 across the ablations; steady-state flow, inlet temperature and pump head within a few percent and about ±7 K; fuel temperature rise under-predicted by 24 %. HTGR passive decay heat: 254.0 kW against Hu et al.'s 206 kW, +23.3 % (since 2026-09-28, `examples/htgr_sim_v1/physics/decay_heat_removal.rs`). Double-pipe HF to LF ([`verification_and_validation/hf_lf_double_pipe_gh1178.md`](verification_and_validation/hf_lf_double_pipe_gh1178.md), 2026-11-03/04): uncalibrated 1-D UA below the CFD's at 44 of 44 points, by 1.1 % to 21.4 %. CIET in the browser ([`verification_and_validation/ciet_web/README.md`](verification_and_validation/ciet_web/README.md), 2026-10-17): the browser plant equals the desktop stepper field for field. Further records in `verification_and_validation/` (KOH, MFK, filtering, OpenDA). None of this is a validation of a plant. |
| Human reviews and corrections | 0 functions carry a kovan human-review stamp. One entry in `docs/human-corrections-to-ai-work.md` (2026-08-12: the excursion overlay drew an explosion an HTGR cannot have). `AGENTS.md` records three defects the maintainer caught on 2026-08-14 that the tests did not (a second-law violation, a feedforward controller, a `c_p` shortcut), and the rejected calibration of an axial length to Hu's 206 kW. Struck-through corrections below (pebble-bed core, MSRE default kinetics, module locations). |

**Status:** the MSRE, EBR-II, HTR-10 data and double-pipe modules carry
recorded comparisons against published or code-to-code references; no
simulator is validated or human-reviewed, and every example is an offline
demonstration.

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
| `color_maps/` | **Real.** Hot/cold and steam-quality colour functions, ported from the existing validated FHR/CIET simulator examples, and Crameri's Scientific colour maps. ~~**Since 2026-11-02 thin `egui::Color32` wrappers over `bugis::colour_maps`**, where the maps and tables now live byte for byte.~~ **CORRECTED 2026-11-27** — the `Color32` wrappers moved too: `color_maps` is now a re-export of `bugis::colour_maps::color32`, and `src/color_maps/` holds only kovan review records (checked against `src/color_maps/` and `AGENTS.md`, gh:#1112). |
| `components/` | **Real wrappers.** ~~Ten visual process objects, each composing its `tampines` (or `nee_soon`) physics counterpart. `instrumentation` is a deliberate visual-only label/value placeholder — `nee_soon` exposes no instrumentation-readout type to wrap yet.~~ **CORRECTED 2026-11-27** — since the 2026-11-05 split (gh:#1112) this module holds the physics-backed widgets (`PipeVisual`, `PipeComponent`, `ValveVisual`, `ReactorVesselVisual`, `CondenserVisual`, `CoolingTowerVisual`, `HeatExchangerVisual`, `PumpVisual`, `SteamGeneratorVisual`, `TurbineVisual`), `ReactorArchetypeKinetics` and the `excursion` overlay; they draw through `bugis`'s `*Art` types, and the rest of the widget art (including `instrumentation`) is `bugis::components`, re-exported here (checked against `src/components/mod.rs`). |
| `animation/` | **Real.** Flow-tracer kinematics: `TracerTrain`, `residence_time_from_flow`, and the `FlowTracer`/`TravelTime` trait contracts. **CORRECTED 2026-11-27** — moved to `bugis::animation` on 2026-11-02 (gh:#1112) and re-exported here; `src/animation/` holds only kovan review records. |
| `app_scaffold/` | **Real.** `Arc<Mutex<_>>` physics-thread + panel-dispatch pattern, plus the thread-panic "please restart" modal. **CORRECTED 2026-11-27** — moved to `bugis::app_scaffold` on 2026-11-02 (gh:#1112) and re-exported here; `src/app_scaffold/` holds only kovan review records. |
| `htr10/` | Cited HTR-10 design constants and packed-bed correlations (`design`, `kta`, `zbs`, `neutronics`), each with a test against its published value. Not validated. (Row added 2026-11-27: the module was missing from this table.) |
| `twin` | The multifidelity digital-twin template (epic gh:#1743): `MultiFidelityTwin`, the seven-page `twin::shell::TwinShell`, the native `twin::local::LocalHost`; adopted by `msre::twin::MsreTwin` and `ebr2::twin::Ebr2Twin`; example `twin_app`. Not validated. (Row added 2026-11-27.) |
| `headless/`, `opcua_core/`, `ciet_opcua/`, `htgr_opcua/` | Headless driving of a simulator with no GUI; OPC-UA (IEC 62541) server layers for offline demonstration only. Infrastructure, not physics. (Row added 2026-11-27.) |
| `ebr2/` | **AI draft, not validated (gh:#1331).** EBR-II for web simulations: cited SHRT-17 / SHRT-45R data (`design`, `shrt`, `kinetics`, `pump`; AI transcriptions of ANL-ARC-226/228 and IAEA-TECDOC-1819, unreviewed) and a GUI-free, wasm-compilable stepping model (`model::Ebr2Model`: point kinetics, fast-reactor feedback, lumped fuel / core / IHX / pool with TUAS sodium). The IHX thermal-centre height is a required input (not in the sources). Record: `docs/ebr2_library.md`. |
| `msre/` | **AI draft, not validated (gh:#1330).** MSRE in one place for web simulations: cited plant constants (`design`), a GUI-free, wasm-compilable stepping simulator (`model::MsreModel`: circulating-fuel kinetics with fuel and graphite feedback, heat exchanger, coolant loop, radiator, piping lags, from ORNL-TM-1070 and ~~`teh-o-prke`~~ `msre::circulation` (bit-identical to `teh-o-prke`'s kinetics at full flow; **CORRECTED 2026-11-10, gh:#1332**); since gh:#1332 fuel- and coolant-pump trips and start-ups, checked against ORNL-4233 Fig. 24), and re-exports of the workspace's MSRE V&V pieces. ~~Reproduces TM-1647's measured natural period at 7.3 MW to +0.2 %; circulation loss +16 % against TM-1626's measurement.~~ **CHANGED 2026-11-11 (gh:#1330, maintainer: "Let's use ORNL more correct physics"):** the default kinetics are ORNL-TM-1626's importance-weighted line model; TM-1070's well-stirred model is the ablation `MsreConfig::without_line_model()`. With the default, the circulation loss is +8.1 % against TM-1626's measurement (well-stirred +16 %), and TM-1647's natural period is +5.0 % at 7.3 MW and +2.0 % at 1.0 MW (well-stirred +0.2 % and −5.4 %). It costs about 40-60 µs per 0.01 s step (well-stirred about 1 µs). Record: `docs/msre_library.md`. |
| `multifidelity/` | **AI draft, rung 1 of gh:#1177 (gh:#1178).** `double_pipe`: runs the double-pipe conjugate-heat-transfer CFD (`outram-foam-appbuilder-lib`) and the TUAS 1-D shell-and-tube exchanger (one tube) at the same design points, reads Nusselt and friction closures off the CFD, and interpolates them. The committed dataset and the rung-3 interface (`one_d::evaluate(&x, &θ)`) are documented in the module. Infrastructure: no correlation of its own. V&V record: `verification_and_validation/hf_lf_double_pipe_gh1178.md`. |

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

~~Both examples are~~ **CORRECTED 2026-11-27** — there are twelve examples,
not two: eleven declared in `Cargo.toml` (`htgr_sim_v1`, `fhr_sim_v2`,
`distillation_sim_v1`, `widget_studio`, `twin_app`, `ciet_web`, `msre_web`,
`ebr2_web`, `fukushima_web`, `double_pipe_twin_web`, `double_pipe_lf_grid`)
and `examples/hf_lf_double_pipe_dataset.rs`. The two below are the original
pair. Every one is an **offline demonstration model**, not a validated plant
model.

```bash
cargo run --release --example htgr_sim_v1
cargo run --release --example fhr_sim_v2
```

### `htgr_sim_v1`

**Status: ~~scaffold~~ demo** (CHANGED 2026-09-28, maintainer: "no longer a
scaffold, but a demo"). A demo is an offline demonstration for education and
research, not a validated or operational tool (`RESPONSIBLE_USE.md`).

> **Read this first (2026-09-29, gh:#387, #408, #409).**
> - **One delayed-neutron fraction everywhere**, beta_eff = 7.26e-3 (Chen et
>   al. 2009), for the rods, the prompt layer and the delayed bank.
> - **The delayed-neutron precursors start EMPTY** and fill over the first
>   minutes (the longest group's ~56 s half-life sets the scale; within 1 % of
>   equilibrium after 245 s at 10 MW). From a cold start at net 0 $ the power
>   first drops. Early-transient numbers are the precursors filling, not plant
>   behaviour.
> - **Decay heat starts at equilibrium**, a conservative choice (more decay
>   heat early).
> - **Rod worth vs the feedback reference is demo-grade** (cold-clean worth
>   against an illustrative 950 K feedback zero; gh:#408).
> - **The reactor building is not credited** (conservative): the primary leak
>   goes straight to the stack (gh:#409).

A helium-cooled, graphite-moderated ~~prismatic-block~~ **pebble-bed**
(HTR-10-shaped; CORRECTED 2026-09-28 — the prismatic core was replaced by the
pebble bed in `physics/reactor_model/one_node.rs`) HTGR: reactor kinetics, a
helium primary loop, and a steam secondary loop, drawn entirely on this crate's
reusable widgets.

**What is real:**

> **Not re-checked in full (2026-09-28).** Parts of this list predate the
> pebble-bed rewrite and are known stale — e.g. ~~Haaland pipe friction~~ was
> removed in favour of the KTA packed-bed correlation, and the IHX is now a
> nodalised counter-flow steam generator (`physics/steam_generator.rs`), not an
> effectiveness-NTU lump. The module doc comments are the authority.

- **Core energy chain (2026-09-28, gh:#360):** fission (prompt share) and decay
  heat are deposited in a **fuel node** (the TRISO kernels and coatings, the
  Nordheim-Fuchs node), which conducts to the **pebble bed** through the
  resolved two-zone pebble; the bed loses to the helium and, in parallel, to
  the reflector -> vessel -> RCCS chain. Graphite `c_p` and conductivity come
  from `tuas_boussinesq_solver` (`NuclearGraphiteMatrixA3HighTemp`, IG-110 for
  the reflector annulus); conductivity above 2000 K is **extrapolated**.
- Kinetics wired to `teh-o-prke`'s prompt-excursion layer and
  `DelayedNeutronLayer`.
- Helium `c_p` and density from the CoolProp-derived Helmholtz EOS
  (`outram-park-fork-coolprop`, helium after Ortiz-Vega et al.), re-evaluated
  every step at the current bulk mean temperature.
- Water/steam properties from IAPWS-IF97 (`tampines-steam-tables`) throughout —
  every state is a genuine $(p,h)$ / $(p,s)$ / saturation flash.
- ~~**Two-way loop coupling.** Each step hands the secondary's saturation
  temperature to the primary as the IHX cold-side pinch. The IHX duty follows
  the effectiveness-NTU result for one isothermal side,~~ **CORRECTED
  2026-11-27** — the IHX is now the nodalised counter-flow steam generator
  of `physics/steam_generator.rs` (hot fluid, tube metal and water/steam
  arrays coupled node by node), as the note above already said; the lumped
  relation below is the superseded one, kept for the record:

  $$Q = \varepsilon \dot{m} c_p (T_{\mathrm{out}} - T_{\mathrm{sink}}), \quad \varepsilon = 1 - e^{-\mathrm{NTU}}$$

  so duty cannot exceed what the temperature difference and $UA$ support. The
  resulting IHX helium outlet becomes the next core inlet, making the core
  inlet a computed loop variable rather than a fixed constant.
- ~~Darcy-Weisbach loop pressure drop with a Haaland friction factor, and the
  circulator power to sustain it.~~ **CORRECTED 2026-11-27** — the Haaland
  pipe friction was removed with the prismatic core; the bed's share of the
  loop loss is the KTA packed-bed correlation and the rest is the published
  component sum (`physics/primary_loop.rs` module doc).
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
no GUI, so it compiles ~~everywhere the workspace targets~~ on every native
target, including headless for
`aarch64-linux-android` (**CORRECTED 2026-10-17**: it was gated off `wasm32`
entirely; since gh:#924 only its `state` submodule builds there, for the
browser demo below); `async-opcua` uses RustCrypto rather than `openssl-sys`
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

### In the browser (gh:#924)

The same simulator runs on the Pages site at `demos/ciet/`: a plain-JS page that
only draws, and the v2 physics compiled to wasm on three Web Workers, mirroring
v2's threads (a plant worker for the heat transfer and real-time pacing, and a
DRACS-flow and a primary-flow worker for the flow solves). One copy of the
physics: the loop body moved into `full_simulation/stepper.rs` (`CietLoop`),
which the desktop thread and `examples/ciet_web` both call. Build with
`web/ciet/build.sh`; record and measurements in
[`verification_and_validation/ciet_web/README.md`](verification_and_validation/ciet_web/README.md).

```bash
cargo run --release --example ciet_web -- --headless 600 10 8   # CSV trace, 8 kW
crates/outram-park-digital-twin-engine/web/ciet/build.sh        # the browser demo
```

### Two more browser demos, moved from `dhoby-ghaut` (2026-11-05)

`fukushima_web` (the Fukushima Daiichi Unit 1 station blackout, plant model
`sembawang::bwr_mark1`, published password-gated at `demos/fukushima/`) and
`double_pipe_twin_web` (the double-pipe high-to-low-fidelity twin, gh:#1182,
`demos/double-pipe-twin/`, record
[`verification_and_validation/double_pipe_twin_web_gh1182/`](verification_and_validation/double_pipe_twin_web_gh1182/README.md))
moved here with their `web/` pages (maintainer, 2026-11-05: "if web examples live in dhoby ghaut, and this causes the dependency cycle, let the web examples deploy from digital twin engine instead"). Both are
built on `bugis::web_demo`; the published URLs are unchanged.

```bash
crates/outram-park-digital-twin-engine/web/fukushima/build.sh
crates/outram-park-digital-twin-engine/web/double_pipe_twin/build.sh
```

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
are gated to match, so ~~`cargo check --target aarch64-linux-android` still
passes with the library reduced to `animation/`, which is deliberately
`egui`-free.~~ **CORRECTED 2026-11-27** — per this crate's `AGENTS.md`
(corrected 2026-09-21, GitHub #239), `cargo check --release --target
aarch64-linux-android` **fails**: `app_scaffold` (now `bugis::app_scaffold`,
re-exported ungated) uses `egui` without an Android gate, so the library does
not reduce to `animation/`. Not re-measured here.

## Build & test

```bash
cargo build --release -p outram-park-digital-twin-engine
cargo test --release -p outram-park-digital-twin-engine --lib --tests --examples
```

## Licence

~~GPL-3.0~~ **AGPL-3.0-only** (relicensed 2026-10-20, gh:#973). See the workspace root for the full licence text and
`RESEARCH_INTEGRITY_AND_PROVENANCE.md` for attribution expectations.

## Copyright

Copyright (C) 2026 Ong Kay Chen Theodore, Professor Per F. Peterson,
University of California, Berkeley Thermal Hydraulics Lab,
Singapore Nuclear Research and Safety Institute (SNRSI),
National University of Singapore (NUS), Repository Contributors.
