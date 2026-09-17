# CIET Educational Simulator v2 — OPC-UA interface reference

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

This is the deep reference for the OPC-UA (IEC 62541) interface of the **CIET
Educational Simulator v2**. For "how do I run it and click things", start at the
simulator's own README:
[`src/bin/ciet_educational_simulator_v2/README.md`](../src/bin/ciet_educational_simulator_v2/README.md).

**Two things to know before you read any further:**

1. **On campus or enterprise WiFi this will not work.** Use a phone hotspot or a
   home router. See [Troubleshooting](#troubleshooting).
2. **There is no authentication and no encryption.** Anyone on your network can
   read every value and write every control. See
   [The threat model, deliberately unaddressed](#the-threat-model-deliberately-unaddressed).

> **Scope limit (`RESPONSIBLE_USE.md`).** OPC-UA is a plant-connectivity
> protocol, so the boundary needs saying out loud: this interface exists so an
> **offline** educational simulator can be driven by standard OPC-UA tooling on
> a bench or in a classroom. It must **never** be connected to live operational
> systems, plant systems, safety-critical infrastructure, real-time plant
> monitoring, or institutional production systems. Its outputs are not
> authoritative for any operational, licensing, or safety purpose.

## Contents

- [Where the code lives](#where-the-code-lives)
- [Endpoint and namespace](#endpoint-and-namespace)
- [The full node table](#the-full-node-table)
- [Write semantics: clamping, NaN, and interactions](#write-semantics-clamping-nan-and-interactions)
- [Discovery: cooperative announcement, not scanning](#discovery-cooperative-announcement-not-scanning)
- [The threat model, deliberately unaddressed](#the-threat-model-deliberately-unaddressed)
- [Troubleshooting](#troubleshooting)
- [Verification status](#verification-status)

## Where the code lives

| Path | Role |
|---|---|
| `src/ciet_opcua/node_map.rs` | **The single source of truth.** Three enums — `CietSignal`, `CietControl`, `CietSwitch` — define every variable. The server's address space, its read/write callbacks, the GUI's node table, and the demo client's variable list are all derived from them |
| `src/ciet_opcua/state.rs` | `CietState`, the flat plant snapshot shared between the physics thread and the server thread, plus the clamping setters and the envelope constants |
| `src/ciet_opcua/server.rs` | The OPC-UA server, run on its own thread with its own tokio runtime |
| `src/ciet_opcua/discovery.rs` | Cooperative mDNS announce (simulator side) and browse (client side) |
| `src/ciet_opcua/pki_paths.rs` | Platform-appropriate PKI directory resolution |
| `src/bin/ciet_educational_simulator_v2/` | The simulator binary: physics, GUI, and the server host |
| `src/bin/ciet_v2_opcua_client/` | The bundled egui demo client (desktop only) |

The `ciet_opcua` module lives in the crate **library**, not in either binary,
because both binaries need it. It contains no physics and no GUI, so it compiles
everywhere the workspace targets — including headless for
`aarch64-linux-android` with no target gate. `async-opcua` was chosen for
exactly that reason: its crypto is RustCrypto, not `openssl-sys`.

**Adding a variable** means adding one enum variant and filling in its `match`
arms; the compiler then points at every place that must be updated. That
exhaustiveness is why these are enums rather than a table of trait objects — see
the workspace Rust design rules (no `Box<dyn Trait>` for dispatch).

## Endpoint and namespace

| Item | Value |
|---|---|
| Endpoint URL | `opc.tcp://<host>:4840/ciet` |
| Default port | `4840` — the IANA-registered `opcua-tcp` port |
| Endpoint path | `/ciet` |
| Namespace URI | `urn:outram-park:ciet-educational-simulator-v2` |
| Security policy | `None` |
| User token | Anonymous |
| Folders in the address space | `Outputs` (read-only), `Controls` (writable) |
| Variable count | 36 — 21 signals, 8 controls, 7 switches |

### Resolve the namespace index; do not hard-code it

The namespace **index** is assigned by the server at start-up. In practice it is
`2` — namespace `0` is the OPC-UA core namespace and `1` is the server's own —
but that is a convention of the current start-up order, not a guarantee.

Resolve it from the URI. Every client stack has a call for this
(`get_namespace_index` in `opcua-asyncio`, the namespace array in a raw browse).
The simulator's OPC-UA GUI page prints the running index, so you can always see
what it actually is.

### Node identifiers are strings

Identifiers are string `NodeId`s rather than numeric ones, so a client can
address any variable by name without browsing first:

```text
ns=2;s=CIET.Heater.PowerKw
ns=2;s=CIET.Temperature.BT12HeaterOutletDegC
ns=2;s=CIET.Control.CtahPumpPressurePascals
ns=2;s=CIET.Control.CtahBranchBlocked
```

The **browse name** of a node is the last dot-separated segment of its
identifier — `BT12HeaterOutletDegC`, `CtahPumpPressurePascals`. These
identifiers are treated as **public API**: client configurations and saved trend
definitions reference them by name, so they are not renamed casually.

## The full node table

Transcribed from `src/ciet_opcua/node_map.rs`. If this table and that file ever
disagree, the file is right.

### Read-only signals — `CietSignal`, 21 variables

OPC-UA type `Double`, access `CurrentRead`.

| Node identifier | Browse name | Display name | Unit | Meaning |
|---|---|---|---|---|
| `CIET.Heater.PowerKw` | `PowerKw` | Heater power | kW | Power **actually applied** this timestep. Differs from the set point when the over-temperature killswitch has tripped |
| `CIET.Temperature.BT11HeaterInletDegC` | `BT11HeaterInletDegC` | BT-11 heater inlet | degC | Heater inlet bulk temperature |
| `CIET.Temperature.BT12HeaterOutletDegC` | `BT12HeaterOutletDegC` | BT-12 heater outlet | degC | Heater outlet bulk temperature |
| `CIET.Temperature.BT43CtahInletDegC` | `BT43CtahInletDegC` | BT-43 CTAH inlet | degC | CTAH inlet bulk temperature |
| `CIET.Temperature.BT41CtahOutletDegC` | `BT41CtahOutletDegC` | BT-41 CTAH outlet | degC | CTAH outlet bulk temperature |
| `CIET.Temperature.BT60DhxTubeInletDegC` | `BT60DhxTubeInletDegC` | BT-60 DHX tube inlet | degC | DHX tube-side inlet bulk temperature |
| `CIET.Temperature.BT21DhxTubeOutletDegC` | `BT21DhxTubeOutletDegC` | BT-21 DHX tube outlet | degC | DHX tube-side outlet bulk temperature |
| `CIET.Temperature.BT21DhxShellInletDegC` | `BT21DhxShellInletDegC` | BT-21 DHX shell inlet | degC | DHX shell-side inlet bulk temperature |
| `CIET.Temperature.BT27DhxShellOutletDegC` | `BT27DhxShellOutletDegC` | BT-27 DHX shell outlet | degC | DHX shell-side outlet bulk temperature |
| `CIET.Temperature.BT65TchxInletDegC` | `BT65TchxInletDegC` | BT-65 TCHX inlet | degC | TCHX inlet bulk temperature |
| `CIET.Temperature.BT66TchxOutletDegC` | `BT66TchxOutletDegC` | BT-66 TCHX outlet | degC | TCHX outlet bulk temperature |
| `CIET.Flow.FM40CtahBranchKgPerS` | `FM40CtahBranchKgPerS` | FM-40 CTAH branch flow | kg/s | CTAH-branch mass flowrate. **Signed** — negative is reverse flow |
| `CIET.Flow.FM20DhxBranchKgPerS` | `FM20DhxBranchKgPerS` | FM-20 DHX branch flow | kg/s | DHX-branch mass flowrate |
| `CIET.Flow.FM60DracsKgPerS` | `FM60DracsKgPerS` | FM-60 DRACS loop flow | kg/s | DRACS-loop mass flowrate **magnitude** |
| `CIET.Controller.CtahHtcWattPerM2K` | `CtahHtcWattPerM2K` | CTAH air-side HTC | W/(m^2 K) | Air-side heat transfer coefficient commanded by the CTAH PID controller |
| `CIET.Controller.TchxHtcWattPerM2K` | `TchxHtcWattPerM2K` | TCHX air-side HTC | W/(m^2 K) | Air-side heat transfer coefficient commanded by the TCHX PID controller |
| `CIET.Temperature.TopMixingNodeDegC` | `TopMixingNodeDegC` | Top mixing node | degC | Mixing node joining branches 5a / 5b / 4 |
| `CIET.Temperature.BottomMixingNodeDegC` | `BottomMixingNodeDegC` | Bottom mixing node | degC | Mixing node joining branches 17a / 17b / 18 |
| `CIET.Time.SimulationSeconds` | `SimulationSeconds` | Simulation time | s | Simulated time elapsed |
| `CIET.Time.ElapsedSeconds` | `ElapsedSeconds` | Wall-clock time | s | Wall-clock time elapsed. Compare with simulated time to see whether the run is keeping up with real time |
| `CIET.Time.CalcMs` | `CalcMs` | Timestep cost | ms | Wall-clock cost of the last timestep |

### Writable continuous controls — `CietControl`, 8 variables

OPC-UA type `Double`, access `CurrentRead | CurrentWrite`. Writes are clamped to
the range shown.

| Node identifier | Display name | Unit | Min | Max | Notes |
|---|---|---|---|---|---|
| `CIET.Control.HeaterPowerKw` | Heater power set point | kW | 0 | 15 | Overwritten every timestep while advanced heater control is on |
| `CIET.Control.CtahPumpPressurePascals` | CTAH pump pressure | Pa | -17000 | 17000 | The forced-circulation driver. Negative reverses flow direction |
| `CIET.Control.Bt41CtahOutletSetPointDegC` | CTAH outlet set point (BT-41) | degC | 15 | 120 | Target for the CTAH air-side PID controller |
| `CIET.Control.Bt66TchxOutletSetPointDegC` | TCHX outlet set point (BT-66) | degC | 15 | 120 | Target for the TCHX air-side PID controller |
| `CIET.Control.HeaterSteadyStatePowerKw` | Heater steady-state power | kW | 0 | 15 | The mean power used by advanced heater control |
| `CIET.Control.FrequencyResponseAmplitudeKw` | Frequency-response amplitude | kW | 0 | 4 | Peak amplitude of the sinusoidal power perturbation |
| `CIET.Control.FrequencyResponseAngularVelocityRadPerS` | Frequency-response angular velocity | rad/s | 0 | 10 | Angular frequency of that perturbation |
| `CIET.Control.TimestepSeconds` | Solver timestep | s | 0.001 | 0.1 | Only honoured in slow-motion mode. 0.1 s is a hard stability ceiling |

### Writable switches — `CietSwitch`, 7 variables

OPC-UA type `Boolean`, access `CurrentRead | CurrentWrite`.

| Node identifier | Display name | Effect when `true` |
|---|---|---|
| `CIET.Control.AdvancedHeaterControlOn` | Advanced heater control | Heater power is driven each timestep by the steady-state + frequency-response settings. Direct writes to `HeaterPowerKw` are overwritten |
| `CIET.Control.FrequencyResponseOn` | Frequency response | Adds the sinusoidal perturbation on top of the steady heater power |
| `CIET.Control.CtahBranchBlocked` | CTAH branch blocked | Blocks flow through the CTAH branch, as if a valve were shut |
| `CIET.Control.DhxBranchBlocked` | DHX branch blocked | Blocks flow through the DHX branch |
| `CIET.Control.FastForwardOn` | Fast forward | Runs faster than real time where the machine allows it |
| `CIET.Control.SlowMotionOn` | Slow motion | Runs slower than real time, honouring the requested timestep |
| `CIET.Control.CoarseHeaterMesh` | Coarse heater mesh (8 nodes) | Uses the coarse 8-node heater mesh instead of the fine 15-node one. Cheaper per timestep — useful on slow hardware and on Termux |

## Write semantics: clamping, NaN, and interactions

### Clamping, not rejection

Every continuous control carries an inclusive envelope, and **an out-of-range
write is honoured at the nearest limit** rather than returning a bad-status
code. Write 1000 kW of heater power and the simulator stores 15 kW.

Why clamp rather than reject? Because this control surface is reachable from the
network, and the guarantee worth having is *"the solver cannot be pushed
somewhere non-physical or unstable, no matter what a client sends"*. A rejected
write leaves a client guessing; a clamped write plus a read-back tells it
exactly where it ended up.

**Always read back after writing.** The read-back is the only way to learn what
was actually accepted.

Where the limits come from:

| Control | Envelope | Reason |
|---|---|---|
| Heater power (both) | 0 to 15 kW | CIET's heater is rated near 10 kW; 15 kW leaves transient headroom. The real protection is the killswitch: 150 degC at heater inlet/outlet, 160 degC in any fluid node, 350 degC in any shell node |
| CTAH pump pressure | ±17000 Pa | CIET's CTAH pump cannot deliver more than about 17 kPa. v1 applied the same bound in its GUI slider; v2 applies it in the setter so a remote write cannot bypass it |
| Outlet set points | 15 to 120 degC | The loop's practical window. Therminol VP-1 / Dowtherm A is liquid well below 21 degC, but CIET is an atmospheric loop and the killswitch trips at 150 degC, so set points above 120 degC are not useful |
| Frequency-response amplitude | 0 to 4 kW | Keeps the perturbation inside the heater envelope when added to a realistic steady power |
| Frequency-response angular velocity | 0 to 10 rad/s | Well above the loop's interesting dynamics; beyond it the perturbation is faster than the solver can resolve meaningfully |
| Timestep | 0.001 to 0.1 s | Above 0.1 s the advection Courant number in the shortest loop component exceeds unity and the explicit advection coupling goes unstable. Enforced by the physics thread regardless of what a client requests |

### NaN writes are ignored

A `NaN` write leaves the previous value in place and returns it. A `NaN` set
point would propagate through the solver and destroy the entire run, so it is
treated as "no change" rather than stored.

Note that `f64::INFINITY` is *not* a special case — it clamps to the maximum
like any other out-of-range number.

### Interactions that will surprise you

- **`AdvancedHeaterControlOn` shadows `HeaterPowerKw`.** While it is on, the
  heater driver rewrites the power set point every timestep. To change power,
  write `HeaterSteadyStatePowerKw` (and, for a perturbation,
  `FrequencyResponseAmplitudeKw` plus
  `FrequencyResponseAngularVelocityRadPerS`, with `FrequencyResponseOn` set).
- **`TimestepSeconds` only bites in slow motion.** Outside slow-motion mode the
  pacing logic chooses the timestep; the request is stored but not used.
- **`FastForwardOn` and `SlowMotionOn` are independent booleans**, not two
  values of one mode. Setting both is accepted; the physics thread's pacing
  logic decides what that means. Pick one.
- **`CIET.Heater.PowerKw` (signal) and `CIET.Control.HeaterPowerKw` (control)
  are different nodes.** The signal is what was applied; the control is what was
  asked for. They diverge when the killswitch trips.
- **The CTAH pump pressure round-trips through `f32`** internally, so a
  read-back may differ from what you wrote in the last few decimal places.

### Threading model

The physics thread and the OPC-UA server thread share one `CietState` behind a
read-write lock. The physics thread publishes each timestep's results in a
single write, holding the lock briefly; server read callbacks take a read lock,
and write callbacks take a write lock and apply the clamping setter.

Consequences worth knowing:

- A client's write takes effect on the **next** timestep, not instantly.
- Values read in one OPC-UA read request are not guaranteed to be from the same
  timestep as each other unless the client batches them into a single read.
- **Advanced heater control and frequency response run in the physics thread**,
  not in the GUI's repaint callback as they did in v1. That is what makes them
  usable remotely and in headless mode — in v1 they only advanced while the
  window was being repainted.

## Discovery: cooperative announcement, not scanning

### How it works

| Side | Behaviour |
|---|---|
| Simulator | **Announces** itself on the local link via mDNS / DNS-SD as `_opcua-tcp._tcp.local.`, with a TXT record marking it as a CIET v2 instance. The address and port come from the DNS-SD service record itself |
| Demo client | **Listens** for those announcements and lists what it hears |

`_opcua-tcp._tcp.local.` is not an invention of this project — it is the service
type the OPC Foundation's Local Discovery Server with Multicast Extension
(LDS-ME) uses, so a CIET v2 instance is visible to any tool that already browses
for OPC-UA servers that way.

`--no-advertise` turns the announcement off. The endpoint keeps working; it just
has to be typed in by hand.

### Why there is no scanner, and never will be

**This workspace does not ship, and will not ship, a port scanner or a subnet
sweeper.** Not as a hidden flag, not as a debug tool, not as a "just for the lab
network" convenience.

The reasons are not technical:

- Unsolicited scanning of a network you do not administer breaches
  institutional acceptable-use policy. For an NUS-affiliated project that is a
  compliance matter, not a style preference.
- It is out of scope per the workspace `RESPONSIBLE_USE.md`, which limits this
  project to education, research, capability building, and V&V.
- Announcement is strictly better for the actual use case anyway. A simulator
  that wants to be found says so; one that does not is left alone. Scanning
  inverts that.

The design consequence is that discovery is **best-effort by construction**. If
the network drops multicast, discovery finds nothing, and the correct response
is the manual endpoint field — not a more aggressive search.

### Manual endpoint entry

The demo client has a field for typing an endpoint URL directly:

```text
opc.tcp://192.168.43.17:4840/ciet
```

The simulator's OPC-UA GUI page prints the URL to use, including the machine's
LAN address rather than `0.0.0.0`, so another device on the same network can
reach it. `local-ip-address` is what resolves that address.

## The threat model, deliberately unaddressed

The server runs **`SecurityPolicy::None` with anonymous access**. Stated
plainly, so nobody has to infer it:

| Property | Status |
|---|---|
| Authentication | **None.** No username, no password, no certificate, no token |
| Encryption | **None.** Traffic is plaintext on the wire |
| Message signing | **None.** Nothing detects tampering in flight |
| Authorisation | **None.** Every connected client can write every writable node |
| Audit trail | **None.** Nothing records who wrote what |
| Rate limiting on writes | **None** beyond whatever the OPC-UA stack imposes |
| Trust list / certificate validation | **Not in use**, though a PKI directory is created |

What that means concretely: anyone who can reach the port can set heater power,
drive the CTAH pump in either direction, move both outlet-temperature set
points, block either branch, change the solver timestep, and read every
temperature and flowrate.

**This is intentional, and it is not a bug report waiting to be filed.** The
simulator is a teaching demonstrator meant to be tried in ten minutes. Making
"point UaExpert at it and poke the loop" trivial is worth more here than a PKI
nobody would configure for a classroom demo. Hardening — certificates, a trust
list, user tokens, an audit trail, per-node authorisation — is explicitly out of
scope; the maintainer's position is that this is a demo, and the PKI story is
left to people who want to work on it.

What the design *does* guarantee, and it is worth being precise about the
difference: **a malicious write cannot make the simulator compute nonsense.**
Every writable node is clamped to a documented envelope and `NaN` is refused, so
the worst a hostile client achieves is an annoying but physically bounded
simulation. That is robustness of the solver, **not** security of the interface.
Do not confuse the two.

The only real mitigations available to you are choices about *where* you run it:

| Choice | Effect |
|---|---|
| `--bind 127.0.0.1` | Only clients on this machine can connect. The tightest option, and enough for single-machine demos |
| Phone hotspot | A small network you control, with a known device list |
| Home router / lab bench | Same idea, slightly larger blast radius |
| `--no-advertise` | Stops the announcement. **Not a security control** — the port is still open and trivially reachable by anyone who guesses 4840 |
| Public WiFi | Don't |
| Internet-reachable, or an institutional production network | Out of scope, and out of bounds per `RESPONSIBLE_USE.md` |

### PKI directory

A PKI directory is created for the OPC-UA stack even though certificate
security is not in use:

| Platform | Path |
|---|---|
| Linux, macOS, Termux | `~/.outram-park/ciet-v2-opcua-pki` |
| Windows | `%APPDATA%\outram-park\ciet-v2-opcua-pki` |

Resolution goes through the `directories` crate, so it follows platform
conventions rather than hard-coded paths.

## Troubleshooting

### The client's discovery list is empty, or the connection times out

**Are you on campus, university, corporate, or hotel WiFi? Then that is the
answer, and there is nothing on either machine to fix.** Those networks
routinely enable **client isolation** — devices on the same WiFi cannot reach
each other at all — and **filter multicast**, so mDNS announcements never
propagate. Both discovery and the direct OPC-UA connection fail. Typing the
endpoint in manually does not help, because the transport itself is blocked. No
configuration on our side changes this.

**Do this instead:**

1. Start a personal hotspot on a phone. Any phone will do.
2. Join both machines to it.
3. Run the simulator, then the client. It should appear in the discovery list
   within a few seconds.
4. If it still does not appear, open the simulator's OPC-UA page, read the LAN
   endpoint URL, and type it into the client's manual endpoint field.

A home router works exactly as well. And for a single-machine demo, run both
binaries locally and connect to `opc.tcp://127.0.0.1:4840/ciet` — that path
never touches the network.

Only once you are on a network you control is it worth looking at anything else
below.

### Discovery is empty but manual connection works

mDNS is being filtered while unicast TCP is not — common with some VPN clients,
some Docker/WSL bridge setups, and some access points that block multicast but
not client-to-client traffic. Nothing is wrong with the simulator. Use the
manual endpoint field, or move to a hotspot.

Also check you did not start the simulator with `--no-advertise`.

### "Address already in use" on start-up

Something already holds port 4840 — most often a second copy of the simulator, or
a real OPC-UA product installed on the machine (an LDS commonly sits on 4840).

```bash
# Linux: what is on 4840?
ss -ltnp | grep 4840
```

Then either stop it, or move the simulator:

```bash
cargo run --release --bin ciet_educational_simulator_v2 -- --port 4841
```

Remember to point the client at the new port; the announcement carries it, but a
manually typed URL will not.

### A firewall is blocking it (on a network you control)

The simulator needs inbound TCP on its OPC-UA port, and mDNS needs UDP 5353.

```bash
# firewalld
sudo firewall-cmd --add-port=4840/tcp
sudo firewall-cmd --add-service=mdns

# ufw
sudo ufw allow 4840/tcp
sudo ufw allow 5353/udp
```

On Windows, the first run usually raises a Windows Defender Firewall prompt —
allow it on **private** networks only. If it was denied once, the rule sticks and
must be changed in the firewall settings.

This is only worth doing on your own machine and your own network. On campus
WiFi it will not help, because the block is upstream of both machines.

### It connected, but the value I write does not stick

In order of likelihood:

1. **It was clamped.** Read the node back and compare with the envelope in the
   table above.
2. **It is being overwritten.** `AdvancedHeaterControlOn` rewrites
   `HeaterPowerKw` every timestep — write `HeaterSteadyStatePowerKw` instead.
3. **It only applies in a mode you are not in.** `TimestepSeconds` is only
   honoured in slow-motion mode.
4. **You wrote `NaN`.** It is ignored by design. Some client UIs send an empty
   numeric field as `NaN`.
5. **You wrote the wrong node.** `CIET.Heater.PowerKw` is a read-only signal;
   the writable one is `CIET.Control.HeaterPowerKw`.

### `BadNodeIdUnknown`

Almost always a hard-coded namespace index that does not match the running one.
Resolve the index from `urn:outram-park:ciet-educational-simulator-v2` instead of
assuming `2`, and check the identifier's spelling against the table above —
identifiers are case-sensitive.

### The simulation is falling behind real time

Compare `CIET.Time.SimulationSeconds` with `CIET.Time.ElapsedSeconds`, and watch
`CIET.Time.CalcMs`. If a timestep costs more wall-clock time than it advances
simulated time, the machine cannot keep up. Set
`CIET.Control.CoarseHeaterMesh` to `true` for the cheaper 8-node heater mesh —
this is the usual first move on modest hardware and on Termux.

### Termux specifics

Run the simulator headless; the GUI is not available on Android:

```bash
cargo run --release --bin ciet_educational_simulator_v2 -- --headless
```

On Android the binary selects headless mode automatically, and
`ciet_v2_opcua_client` is a desktop-only stub — connect from a desktop machine
or use a third-party OPC-UA client. Note that Android's hotspot and WiFi
behaviour varies by vendor; if a phone is both the hotspot and the simulator
host, check that the hotspot allows client-to-client traffic.

**This path is unverified — see below.**

## Verification status

Be careful about what is and is not established here.

**Interface unit tests — actually run.** `src/ciet_opcua/node_map.rs` carries
tests, each documenting its methodology and its result per the workspace V&V
documentation rule:

| Test | Checks |
|---|---|
| `node_identifiers_are_unique` | All 36 identifiers distinct — a collision would silently merge two variables |
| `every_control_clamps_out_of_range_writes` | Each of the 8 controls clamps at both ends of its envelope |
| `nan_writes_are_ignored` | A `NaN` write leaves the prior value in place and finite, for all 8 controls |
| `every_switch_round_trips` | All 7 switches latch `true` and `false`, including the boolean-to-`HeaterType` mapping |
| `browse_names_are_single_segments` | All 36 browse names non-empty and dot-free, as OPC-UA requires |

These verify the **interface contract**. They say nothing whatsoever about
whether the physics behind the nodes is right.

**Port equivalence, v1 to v2 — NOT verified.** No side-by-side run of v1 and v2
on identical inputs has been performed or compared. Until that exists, v2's
numbers should not be treated as inheriting v1's validation.

**v1 physics validation — exists, and belongs to v1.** See
`crates/tuas_boussinesq_solver/verification_and_validation/` for what was
checked and against what. Those records describe v1.

**Termux / Android — NOT verified.** An `aarch64-linux-android` `cargo check` is
a **proxy** only: it compiles against the Android target from a host. The
**authoritative** check is a native on-device Termux build (`cargo build` /
`cargo test` inside Termux), and that has **not** been run for these binaries.
Do not report Termux support for CIET v2 as confirmed. Workspace-wide tracking
is in the `op-zfr` "Android support" epic in beads.

**Third-party client interoperability — not tested here.** The server is a plain
IEC 62541 server with no vendor extensions, and standard clients (UaExpert,
`opcua-commander`, FreeOpcUa / `opcua-asyncio`, Node-RED's OPC-UA nodes) are
expected to work against it with anonymous / `None` security. No automated
interoperability test exists in this workspace, and the Python example in the
simulator README has not been executed as part of a test suite.

## Licence

GPL-3.0. See the workspace root for the full licence text, and
`RESEARCH_INTEGRITY_AND_PROVENANCE.md` for attribution expectations.
