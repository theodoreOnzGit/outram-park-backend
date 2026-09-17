# CIET Educational Simulator v2

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

**Just here to try the demo?** Two things will save you an evening:
[Connecting over WiFi](#connecting-over-wifi-read-this-first) — it does **not**
work on campus or enterprise WiFi, use a phone hotspot — and
[Security: there is none](#security-there-is-none-and-that-is-on-purpose-read-this-too)
— anyone on your network can drive the simulator, which is fine on a hotspot and
not fine in a café.

## What this is

A real-time **educational** simulator of **CIET** — the Compact Integral
Effects Test facility at UC Berkeley. CIET circulates Therminol VP-1 /
Dowtherm A through a three-branch primary loop (heater branch, CTAH branch,
DHX branch) joined at a top and a bottom mixing node, plus a separate DRACS
natural-circulation loop. Heat leaves the primary loop through the **CTAH**, an
air-cooled heat exchanger, and through the **DHX**, a shell-and-tube exchanger
that couples the primary loop to the DRACS loop; the DRACS loop rejects its
heat through the **TCHX**.

You drive the simulated facility the way an operator would drive the real one:
set heater power, run the CTAH pump for forced circulation or shut it off for
natural circulation, block a branch as though a valve were closed, and watch the
instrumented temperatures (BT-11, BT-12, BT-41, BT-43, BT-60, BT-21, BT-27,
BT-65, BT-66) and flowmeters (FM-40, FM-20, FM-60) respond.

> **Offline demonstration only.** Per the workspace `RESPONSIBLE_USE.md`, this
> simulator is for education, research, capability building and V&V. It is
> **not** for nuclear facility operation, reactor control, licensing decisions,
> safety-critical decision-making, emergency response, safeguards- or
> security-sensitive analysis, real-time plant monitoring, or operational
> digital-twin deployment. The OPC-UA interface described below does **not**
> change that: it exists so standard industrial tooling can drive an offline
> teaching model on a bench, and connecting it to anything live is out of
> bounds.

### v2 versus v1

v1 lives on, unchanged, as an example in the `tuas_boussinesq_solver` crate:

```bash
cargo run --release --example ciet_educational_simulator
```

v2 is a **port of v1** into the `outram-park-digital-twin-engine` crate as a
`[[bin]]` target. The physics is a faithful port of v1's TUAS-backed model —
same components, same correlations, same solver structure. What v2 adds is
everything around the physics:

| v2 addition | Why |
|---|---|
| Embedded **OPC-UA (IEC 62541) server** on a parallel thread | Drive the simulator from any standard OPC-UA client, or log its outputs like a real plant historian would |
| An **OPC-UA page** in the GUI | Shows the live endpoint URL, the running namespace index, and the node table, so connecting needs no reading of source |
| Cooperative **mDNS service advertisement** | Other machines on the same local network can find the simulator without anyone scanning the network |
| A bundled GUI **demo client** (`ciet_v2_opcua_client`) | A working example of the client side, and a quick way to check the server end to end |
| A **headless mode** (`--headless`) | Physics plus OPC-UA server with no GUI at all — this is what makes on-device Termux / Android use possible |
| Frequency-response and advanced heater control moved out of the GUI repaint callback into the **physics thread** | In v1 those drivers only advanced while the GUI was repainting. In v2 they run in the physics loop, so they work when driven remotely and when running headless |

> **Port equivalence is not yet verified.** The maintainer has done validation
> work on **v1's** physics (see `crates/tuas_boussinesq_solver/verification_and_validation/`).
> That work does not automatically transfer: nobody has yet run v1 and v2 side
> by side on the same inputs and compared their trajectories. Treat v2's
> numbers as unverified until that comparison exists and is written up.

## Minimum requirements

- Rust 1.81 or newer.
- A CPU fast enough to keep up with real time (an i7-10875H was comfortable for
  v1).
- 8 GB RAM.
- 1920 x 1080 or larger for the GUI — below that the main page does not lay out
  properly. (The headless mode has no such requirement.)

On Ubuntu / Debian, the non-GUI build dependencies:

```bash
sudo apt install gcc libssl-dev pkg-config
```

OpenBLAS is required on Linux and macOS for the workspace build:

```bash
sudo pacman -S openblas          # Arch / EndeavourOS
sudo apt install libopenblas-dev # Debian / Ubuntu / Mint
```

Some Windows Subsystem for Linux (WSL) versions hit conflicting `egui`
dependency issues that do not appear on native Linux or native Windows. If the
GUI will not build under WSL, use native Windows or `--headless`.

## Running it

Desktop, with the GUI and the OPC-UA server both up:

```bash
cargo run --release --bin ciet_educational_simulator_v2
```

Headless — physics plus OPC-UA server, no window:

```bash
cargo run --release --bin ciet_educational_simulator_v2 -- --headless
```

The bundled demo client (desktop only):

```bash
cargo run --release --bin ciet_v2_opcua_client
```

### Command-line flags

| Flag | Default | Effect |
|---|---|---|
| `--bind <addr>` | `0.0.0.0` | Interface the OPC-UA server listens on. `0.0.0.0` accepts connections from the local network; `127.0.0.1` accepts only clients on this machine |
| `--port <n>` | `4840` | TCP port for the OPC-UA endpoint. 4840 is the IANA-registered `opcua-tcp` port |
| `--no-advertise` | off | Do not announce the simulator over mDNS. The endpoint still works; it just has to be typed in by hand |
| `--headless` | off | Run the physics and the OPC-UA server with no GUI |
| `--help` | — | Print the flag list and exit |

## Connecting over WiFi (read this first)

**On campus or enterprise WiFi, this will not work.** Not "might need
configuring" — will not work. Those networks enable **client isolation**
(devices on the same WiFi cannot talk to each other at all) and/or **filter
mDNS** (service announcements never reach anyone). The simulator will not appear
in the client's discovery list, and typing the endpoint URL in by hand will not
help either, because the connection itself is blocked. Nothing on our side fixes
this, and nothing on your side does either short of the network administrator
changing policy. Do not spend an evening debugging firewalls — there is nothing
to debug.

**The fix: use a phone hotspot.** This is the recommended path, not a fallback.
Every phone can do it, so no special hardware is involved:

1. Turn on the personal hotspot on a phone.
2. Join **both** machines to that hotspot — the one running the simulator and
   the one running the client. (Both on one machine also works; see below.)
3. On the simulator machine:
   `cargo run --release --bin ciet_educational_simulator_v2`
4. On the client machine:
   `cargo run --release --bin ciet_v2_opcua_client`
5. The simulator appears in the client's discovery list within a few seconds.
   Select it and connect.
6. **If it does not appear**, open the simulator's **OPC-UA page**, read the LAN
   endpoint URL off it (it looks like `opc.tcp://192.168.43.17:4840/ciet`), and
   type that into the client's manual endpoint field. Discovery is a
   convenience; the connection does not depend on it.

A **home router** works equally well — same steps, nothing hotspot-specific
about them. And if you only want to try it on one machine, run both binaries
there and connect to `opc.tcp://127.0.0.1:4840/ciet`; that path never touches
the network at all.

## Security: there is none, and that is on purpose (read this too)

**No authentication. No encryption.** The OPC-UA server accepts anonymous
connections with security policy `None`.

That means: **anyone on the same network as you can read every value and write
every control.** Heater power, CTAH pump pressure, both outlet-temperature set
points, branch blocking, the solver timestep — all of it, with no password, no
certificate, and no record of who did it.

This is intentional. It is a teaching demonstrator meant to be tried in ten
minutes, and making "point a client at it and poke the loop" a ten-second
exercise is worth more here than a PKI nobody would set up. You are not looking
at a defect, and you are not expected to harden it — certificates, trust lists,
user tokens and audit trails are explicitly **out of scope**. What you are
looking at is informed consent: now you know, so choose your network
accordingly.

| Where you are | Verdict |
|---|---|
| Phone hotspot, home router, or a lab bench network | **Fine.** This is exactly what it is for |
| One machine only — `--bind 127.0.0.1` | **Fine, and the tightest option.** Nothing leaves the machine |
| Public or untrusted WiFi — cafés, airports, hotels, conference networks | **Don't.** Anyone on that network can drive your simulator |
| Anything reachable from the internet, or an institutional production network | **No.** Out of scope, and out of bounds per `RESPONSIBLE_USE.md` |

The simulator prints a warning banner whenever it is bound to something other
than loopback. That banner is not boilerplate.

## The GUI pages

The page set is carried over from v1, plus the new OPC-UA page:

| Page | What it is for |
|---|---|
| Main page | The whole facility on one schematic: heater, CTAH, DHX, TCHX, both loops, live temperatures and flows |
| CTAH pump | The forced-circulation driver — pump pressure rise, and branch blocking |
| CTAH | The air-cooled heat exchanger and its outlet-temperature controller |
| Heater | Heater power, the nodal temperature profile, and the killswitch state |
| DHX | The primary-to-DRACS shell-and-tube exchanger, shell and tube side |
| TCHX | The DRACS-loop air cooler and its outlet-temperature controller |
| Frequency response and transients | Steady power plus a sinusoidal perturbation, for Bode-style frequency-response experiments and step transients |
| Nodalised diagram | The nodalisation the solver actually uses, next to the SAM diagram replica |
| Online calibration | On-the-fly adjustment of heater and component parameters |
| OPC-UA | Live endpoint URL, running namespace index, node table, and the security warning |
| Citations and disclaimers | Copyright, credits, and the papers to cite |

## The OPC-UA interface

The simulator hosts an OPC-UA server on its own thread, with its own tokio
runtime, alongside the physics thread. Both threads share one plant-state
snapshot behind a read-write lock, so a client reads the same values the GUI
draws and a client's writes land in the same state the solver reads next
timestep.

**Endpoint:**

```text
opc.tcp://<host>:4840/ciet
```

**Namespace URI:**

```text
urn:outram-park:ciet-educational-simulator-v2
```

The namespace **index** is assigned by the server at start-up — usually `2`,
after the OPC-UA core namespace `0` and the server's own namespace `1` — but
that is not guaranteed. **Resolve the index from the namespace URI rather than
hard-coding `2`.** The OPC-UA page in the GUI prints the running index so you
can see what it actually is.

Node identifiers are **strings**, so a client can address a variable by name
without browsing the address space first:

```text
ns=2;s=CIET.Heater.PowerKw
ns=2;s=CIET.Temperature.BT12HeaterOutletDegC
ns=2;s=CIET.Control.CtahPumpPressurePascals
ns=2;s=CIET.Control.CtahBranchBlocked
```

There are **36 variables**: 21 read-only signals, 8 writable continuous
controls, and 7 writable boolean switches. The read-only ones sit under an
`Outputs` folder and the writable ones under a `Controls` folder.

The authoritative definition is the three enums in
`crates/outram-park-digital-twin-engine/src/ciet_opcua/node_map.rs`
(`CietSignal`, `CietControl`, `CietSwitch`). The tables below are transcribed
from that file; a deeper reference, including the discovery design and
troubleshooting, is in
[`crates/outram-park-digital-twin-engine/docs/ciet-v2-opcua.md`](../../../docs/ciet-v2-opcua.md).

### Read-only signals (21)

`Double`, access `CurrentRead`.

| Node identifier | Display name | Unit |
|---|---|---|
| `CIET.Heater.PowerKw` | Heater power | kW |
| `CIET.Temperature.BT11HeaterInletDegC` | BT-11 heater inlet | degC |
| `CIET.Temperature.BT12HeaterOutletDegC` | BT-12 heater outlet | degC |
| `CIET.Temperature.BT43CtahInletDegC` | BT-43 CTAH inlet | degC |
| `CIET.Temperature.BT41CtahOutletDegC` | BT-41 CTAH outlet | degC |
| `CIET.Temperature.BT60DhxTubeInletDegC` | BT-60 DHX tube inlet | degC |
| `CIET.Temperature.BT21DhxTubeOutletDegC` | BT-21 DHX tube outlet | degC |
| `CIET.Temperature.BT21DhxShellInletDegC` | BT-21 DHX shell inlet | degC |
| `CIET.Temperature.BT27DhxShellOutletDegC` | BT-27 DHX shell outlet | degC |
| `CIET.Temperature.BT65TchxInletDegC` | BT-65 TCHX inlet | degC |
| `CIET.Temperature.BT66TchxOutletDegC` | BT-66 TCHX outlet | degC |
| `CIET.Flow.FM40CtahBranchKgPerS` | FM-40 CTAH branch flow | kg/s |
| `CIET.Flow.FM20DhxBranchKgPerS` | FM-20 DHX branch flow | kg/s |
| `CIET.Flow.FM60DracsKgPerS` | FM-60 DRACS loop flow | kg/s |
| `CIET.Controller.CtahHtcWattPerM2K` | CTAH air-side HTC | W/(m^2 K) |
| `CIET.Controller.TchxHtcWattPerM2K` | TCHX air-side HTC | W/(m^2 K) |
| `CIET.Temperature.TopMixingNodeDegC` | Top mixing node | degC |
| `CIET.Temperature.BottomMixingNodeDegC` | Bottom mixing node | degC |
| `CIET.Time.SimulationSeconds` | Simulation time | s |
| `CIET.Time.ElapsedSeconds` | Wall-clock time | s |
| `CIET.Time.CalcMs` | Timestep cost | ms |

Notes worth knowing before you trend these:

- `CIET.Heater.PowerKw` is the power **actually applied** this timestep, which
  differs from the requested set point when the over-temperature killswitch has
  tripped.
- `FM40CtahBranchKgPerS` is **signed** — negative is reverse flow.
  `FM60DracsKgPerS` is a **magnitude**, because the DRACS loop model uses
  absolute flowrates.
- `SimulationSeconds` against `ElapsedSeconds` tells you whether the simulation
  is keeping up with real time; `CalcMs` is the wall-clock cost of the last
  timestep.

### Writable continuous controls (8)

`Double`, access `CurrentRead | CurrentWrite`. Writes are **clamped**, not
rejected — see below.

| Node identifier | Display name | Unit | Range |
|---|---|---|---|
| `CIET.Control.HeaterPowerKw` | Heater power set point | kW | 0 to 15 |
| `CIET.Control.CtahPumpPressurePascals` | CTAH pump pressure | Pa | -17000 to 17000 |
| `CIET.Control.Bt41CtahOutletSetPointDegC` | CTAH outlet set point (BT-41) | degC | 15 to 120 |
| `CIET.Control.Bt66TchxOutletSetPointDegC` | TCHX outlet set point (BT-66) | degC | 15 to 120 |
| `CIET.Control.HeaterSteadyStatePowerKw` | Heater steady-state power | kW | 0 to 15 |
| `CIET.Control.FrequencyResponseAmplitudeKw` | Frequency-response amplitude | kW | 0 to 4 |
| `CIET.Control.FrequencyResponseAngularVelocityRadPerS` | Frequency-response angular velocity | rad/s | 0 to 10 |
| `CIET.Control.TimestepSeconds` | Solver timestep | s | 0.001 to 0.1 |

### Writable switches (7)

`Boolean`, access `CurrentRead | CurrentWrite`.

| Node identifier | Display name |
|---|---|
| `CIET.Control.AdvancedHeaterControlOn` | Advanced heater control |
| `CIET.Control.FrequencyResponseOn` | Frequency response |
| `CIET.Control.CtahBranchBlocked` | CTAH branch blocked |
| `CIET.Control.DhxBranchBlocked` | DHX branch blocked |
| `CIET.Control.FastForwardOn` | Fast forward |
| `CIET.Control.SlowMotionOn` | Slow motion |
| `CIET.Control.CoarseHeaterMesh` | Coarse heater mesh (8 nodes) |

Two interactions to be aware of:

- While `AdvancedHeaterControlOn` is `true`, the heater-power driver overwrites
  `CIET.Control.HeaterPowerKw` every timestep, so direct writes to it are lost.
  Write `HeaterSteadyStatePowerKw` instead.
- `CIET.Control.TimestepSeconds` is only honoured in slow-motion mode, and is
  clamped to the stability ceiling either way.

### Writes are clamped, not rejected

Every writable continuous control has a documented envelope, and a write
outside it is honoured **at the nearest limit**. Send 1000 kW of heater power
and you get 15 kW, with a read-back that tells you so. This is deliberate: the
control surface is reachable from the network, and a bad write must not be able
to push the solver somewhere non-physical or unstable.

A few of the limits have physical reasons behind them:

- **Heater power 0 to 15 kW** — CIET's heater is rated near 10 kW; 15 kW leaves
  headroom for transient experiments. The real protection is the
  over-temperature killswitch, not this bound.
- **CTAH pump pressure ±17000 Pa** — CIET's CTAH pump cannot deliver more than
  about 17 kPa. Negative values reverse the flow direction.
- **Set points 15 to 120 degC** — the loop's practical window. CIET is an
  atmospheric loop and the killswitch trips well before 150 degC, so higher set
  points are not useful.
- **Timestep 0.001 to 0.1 s** — above 0.1 s the advection Courant number in the
  shortest loop component exceeds unity and the explicit advection coupling goes
  unstable. This is a stability ceiling, not a preference.

**`NaN` writes are ignored** and leave the previous value in place. A `NaN` set
point would propagate through the solver and destroy the run.

## Connecting a third-party client

Any standard OPC-UA client works — the server is a plain IEC 62541 server with
no vendor extensions. Clients that people in this workspace have used the
protocol with include **UaExpert**, **`opcua-commander`**, the
**FreeOpcUa / `opcua-asyncio`** Python stack, and **Node-RED**'s OPC-UA nodes.
Connect anonymously with security policy **None**.

A minimal `opcua-asyncio` session that reads the heater outlet temperature and
writes a heater power set point. This snippet is illustrative — it has not been
run as part of a test:

```python
import asyncio
from asyncua import Client, ua

URL = "opc.tcp://192.168.1.42:4840/ciet"
NS_URI = "urn:outram-park:ciet-educational-simulator-v2"

async def main():
    async with Client(url=URL) as client:
        # Resolve the namespace index instead of assuming it is 2.
        ns = await client.get_namespace_index(NS_URI)

        bt12 = client.get_node(ua.NodeId("CIET.Temperature.BT12HeaterOutletDegC", ns))
        print("BT-12:", await bt12.read_value(), "degC")

        power = client.get_node(ua.NodeId("CIET.Control.HeaterPowerKw", ns))
        await power.write_value(ua.DataValue(ua.Variant(8.0, ua.VariantType.Double)))
        print("heater power read-back:", await power.read_value(), "kW")

asyncio.run(main())
```

The read-back matters: because writes are clamped, it is the only way to know
what the simulator actually accepted.

## The bundled demo client and discovery

`ciet_v2_opcua_client` is a small egui application that finds a running
simulator, connects to it, shows the live outputs, and lets you drive the
controls. It is there as a worked example of the client side and as a quick
end-to-end check of the server.

Discovery is **cooperative, not a scan.** The simulator *announces* itself over
mDNS / DNS-SD as `_opcua-tcp._tcp.local.` — the same service type the OPC
Foundation's LDS-ME uses — with a TXT record marking it as a CIET v2 instance.
The client *listens* for those announcements. Nothing is probed, swept, or
scanned.

**This workspace does not ship, and will not ship, a port scanner or subnet
sweeper.** Unsolicited scanning of a network you do not administer breaches
institutional acceptable-use policy, and it is out of scope per
`RESPONSIBLE_USE.md`. If a simulator is not announcing itself, type its
endpoint URL into the client by hand — the client has a manual endpoint-entry
field for exactly that.

If discovery finds nothing, the network is the usual reason — see
[Connecting over WiFi](#connecting-over-wifi-read-this-first) above, and use the
manual endpoint field.

### PKI directory

A PKI directory is created for the OPC-UA stack even though no certificate
security is in use:

| Platform | Path |
|---|---|
| Linux, macOS, Termux | `~/.outram-park/ciet-v2-opcua-pki` |
| Windows | `%APPDATA%\outram-park\ciet-v2-opcua-pki` |

## Termux / Android status

The **non-GUI path is designed to work on-device**. The physics and the whole
OPC-UA layer are pure Rust: `async-opcua` uses RustCrypto rather than
`openssl-sys`, which is why the interface cross-checks cleanly for
`aarch64-linux-android` with no NDK and no system OpenSSL.

The egui / eframe GUI is out of scope on Android per the workspace
Android-portability rule. Consequently, on Android:

- the simulator binary runs **headless automatically** — the OPC-UA server and
  the physics run, no window is opened;
- `ciet_v2_opcua_client` is a **desktop-only stub**. Use a third-party OPC-UA
  client, or run the bundled client from a desktop machine on the same network.

**Verification status — read this before claiming Termux support.** A
`cargo check --target aarch64-linux-android` is only a **proxy**: it compiles
against the Android target from a host. The **authoritative** check is a native
on-device Termux build (`cargo build` / `cargo test` run inside Termux), and
**that has not been run for this binary.** Termux support for CIET v2 is
therefore **unverified**. Workspace-wide Android/Termux tracking lives in the
`op-zfr` "Android support" epic in beads.

## V&V status and known limitations

Nothing here is validated. Concretely:

- **Port equivalence, v1 to v2: not verified.** No side-by-side comparison of
  v1 and v2 trajectories on identical inputs has been run. This is the single
  biggest open item.
- **v1's physics has had validation work**, recorded under
  `crates/tuas_boussinesq_solver/verification_and_validation/`. Read those
  records for what was actually checked and against what. They describe v1, and
  they do not transfer to v2 until the equivalence check above exists.
- **The OPC-UA layer has unit tests, not V&V.** `node_map.rs` carries tests for
  node-identifier uniqueness, clamping of out-of-range writes on all 8 controls,
  `NaN` rejection, boolean round-tripping on all 7 switches, and browse-name
  validity, each documenting its methodology and result. Those are *interface*
  verification — they say nothing about the physics.
- **Frequency-response and natural-circulation validation is outstanding.** The
  intended references are De Wet's and Poresky's frequency-response results, and
  Zweibaum's natural-circulation experimental data. That work has not been done
  for v2.
- **DRACS-loop flow is one-directional.** The model carries absolute DRACS
  flowrates, a simplification inherited from v1. Reverse DRACS flow cannot be
  simulated.
- **Mass and energy balances share one timestep.** Solving them on separate
  timesteps might buy stability and speed, but that is an untested idea, not an
  implemented feature.

## Credits and citations

Heat exchanger, heater, cooler and pump artwork by **DWSIM**, licensed under
GPLv3.

DWSIM has been compared against commercial process simulators in the
literature:

> Tangsriwong, K., Lapchit, P., Kittijungjit, T., Klamrassamee, T., Sukjai, Y.,
> & Laoonual, Y. (2020, March). Modeling of chemical processes using commercial
> and open-source software: A comparison between Aspen Plus and DWSIM. In *IOP
> Conference Series: Earth and Environmental Science* (Vol. 463, No. 1,
> p. 012057). IOP Publishing.

Copyright: Theodore Kay Chen Ong, SiCong Xiao, SNRSI, and Per F. Peterson.

Citations appreciated:

```bibtex
@phdthesis{ong2024digital,
  title={Digital Twins as Testbeds for Iterative Simulated Neutronics Feedback Controller Development},
  author={Ong, Theodore Kay Chen},
  year={2024},
  school={UC Berkeley}
}

@article{ong2024open,
  title={An open-source Thermo-hydraulic Uniphase Advection and Convection Solver for Salt Flows (TUAS)},
  author={Ong, Theodore Kay Chen and Xiao, Sicong and Peterson, Per F},
  journal={International Journal of Advanced Nuclear Reactor Design and Technology},
  volume={6},
  number={4},
  pages={281--301},
  year={2024},
  publisher={Elsevier}
}
```

## Licence

GPL-3.0, without **any** warranty. Results are not guaranteed to be physically
accurate. Use at your own risk.
