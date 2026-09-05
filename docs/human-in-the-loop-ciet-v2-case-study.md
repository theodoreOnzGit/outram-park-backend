# Human-in-the-loop AI agentic coding — a worked case study

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

**Subject:** building the CIET Educational Simulator v2 and its OPC-UA interface
**Date:** 2026-07-28, one session
**Recorded at the maintainer's request** as a concrete example of what
human-in-the-loop AI agentic coding looked like in practice on this project.
Companion to `AI_USAGE.md`, which states the policy; this document is the
evidence of one instance of it.

## Why record this

`AI_USAGE.md` requires that AI-assisted output be treated as untrusted draft
material until a human reviews it. That is easy to state and hard to
characterise. This session is worth writing down because the human
interventions were not stylistic — **three of them changed the architecture, and
one of them fixed a latent bug in code that had been working for years.** That
is a more useful record than "an AI wrote some code and a human read it".

Nothing here is a claim about productivity. It is a record of where the human
mattered.

## What was built

| Piece | Where |
|---|---|
| CIET Educational Simulator v2 | `crates/outram-park-digital-twin-engine/src/bin/ciet_educational_simulator_v2/` |
| OPC-UA interface layer (shared) | `crates/outram-park-digital-twin-engine/src/ciet_opcua/` |
| OPC-UA GUI demo client | `crates/outram-park-digital-twin-engine/src/bin/ciet_v2_opcua_client/` |

v2 is a port of v1 (which remains in place as an example in
`crates/tuas_boussinesq_solver`) with an embedded OPC-UA server on a parallel
thread, cooperative mDNS discovery, a headless mode, and a bundled demo client.

## Division of labour

Work was partitioned across four background agents with **strict file
ownership**, so no two agents could edit the same file: one for the OPC-UA
server layer, one for adapting the copied v1 tree, one for the demo client, one
for documentation. The coordinating model wrote the shared contract first —
the plant-state struct and the node map — because a contract invented
independently by two agents is a contract that disagrees with itself.

The human (maintainer) was not in the loop as a reviewer at the end. He was in
the loop *continuously*, and that is the point of this record.

## The interventions that changed the design

### 1. "Do a recursive copy of the v1 lah, then start editing it as the v2"

The plan on the table was a compact v2: a trimmed UI with three pages, on the
reasoning that a page-for-page port of v1's nine pages was ~11 kLOC of work.
The maintainer replaced that with `cp -ar`.

**Effect.** v2 shipped with v1's *entire* feature set instead of a subset, and
the work took less time, not more — copying and adapting a working tree beats
re-deriving a trimmed version of it. The compact-v2 plan would have quietly
shipped a downgrade.

**Lesson.** The AI had optimised for lines-of-code written. The maintainer
optimised for capability delivered. Those are not the same objective, and the
person who owns the software is better placed to know which one matters.

### 2. "It is better to persist user input in another struct... then periodically try to sync it into CIETState"

The first design had OPC-UA write callbacks mutating the shared plant state
directly. The maintainer proposed a separate struct for remote control requests,
drained into the plant state periodically.

**Effect.** This is the architecture that shipped
(`ciet_opcua/user_controls.rs`). It fixes a **lost-update race**: with writes
going straight into the plant state, a client's write could be silently
discarded, while still returning `Good` to the client, with nothing on the
client side to diagnose. It also takes remote writes off the plant-state lock
entirely, so many connected clients do not serialise against the solver.

**Lesson.** The maintainer diagnosed a concurrency problem from a description of
the design, before any code exhibiting it had been run. Domain ownership beats
code-reading here: he knew how his own GUI wrote to that struct.

### 3. "Don't worry about V&V 1 for 1... It is less important to match controls as tightly as physics"

The AI had proposed verifying the v1-to-v2 port by requiring step-for-step
agreement of the whole state. The maintainer scoped that down: physics
correctness matters, exact control-field agreement does not.

**Effect.** Kept a V&V gate honest and achievable instead of impossible. The
control path *cannot* match v1 exactly, because v2 deliberately fixed v1's
clobbering behaviour — so a strict whole-state criterion would have failed for a
reason that is an improvement.

**Lesson.** The AI was about to define a correctness criterion that its own
(correct) bug fix would violate. Recognising which differences are regressions
and which are fixes is a judgement about intent, not about code.

### 4. Standing corrections

Two more shaped the work without changing architecture: an early instruction to
**bind to all interfaces** rather than loopback (the demo is meant to be reached
over WiFi), and an instruction that the caveats — no security, campus WiFi will
not work, use a phone hotspot — be made **prominent rather than complete**. The
second changed documentation from technically-thorough to actually-useful: the
information had been present, but buried where a new user would not read it
before wasting an afternoon.

## The bug the port surfaced

Porting v1 turned up a real defect in v1 (filed as `op-wqk.13.9`). v1's GUI
cloned the whole shared state early in an egui repaint, mutated a couple of
control fields on the clone, then stored the whole clone back — about twenty
times a second. In v1 this is nearly harmless: the state is a publication
surface, and the physics thread re-derives its outputs from its own component
objects every timestep, so clobbered outputs are restored within one step. That
is presumably why it went unnoticed.

Add remote control and it stops being harmless, because the same struct carries
the *controls* the solver reads. That is what interventions 2 and 4 above
converged on from two directions — the maintainer from knowing his GUI, the port
from having to make remote writes reliable.

Worth noting for the record: **v1 was not modified.** It is the
maintainer-validated reference, and changing it was outside the scope of the v2
request. The finding was filed for his decision rather than acted on.

## Where the AI agents were wrong, and what caught it

Honesty about the failure modes is the useful part of this record.

| Failure | Caught by |
|---|---|
| An agent died mid-run on an API error, having written two of its three files and silently omitting the one everything else linked against | A file-existence check before trusting the "done" report |
| The coordinator's diagnosis of an address-space bug was **wrong in mechanism** — it concluded the variables were never inserted; they were inserted and then *shadowed*, because the diagnostics node manager claimed the namespace when the application URI was set equal to the CIET namespace URI | The agent's own scratch debug test, whose browse output localised it |
| Two unit tests asserted `f64` round-off tolerance on a value stored in an `f32` field, so 0.1 s read back as 0.10000000149011612 | Running the tests |
| A documented control range was wrong (0.001–0.2 s instead of 0.001–0.1 s) | An agent adding assertions that pin the documented numbers |
| The docs agent finished before the code agents and documented target behaviour that did not exist yet | The agent flagging it in its own report, and a later cross-check of all 36 documented node IDs against the code |
| A `--headless` branch with no `return`, relying on a divergent callee | Reading the control flow while wiring an unrelated change |

The pattern: **agents are good at producing plausible work and poor at knowing
when it is wrong.** Every item above was caught by something mechanical — a
compiler, a test, a file listing, a cross-check — not by an agent's
self-assessment. Two agents reported "done" while their work was incomplete or
incorrect.

## What was verified, and what was not

Verified by execution:

- 97 tests pass in release mode (55 library, 3 simulator binary, 37 client
  binary, 2 integration).
- The OPC-UA interface round-trips end to end over a real TCP socket: namespace
  resolved by URI, all 36 nodes reachable and correctly typed, writes reaching
  plant state, the safety envelope holding over the wire, and a stale
  full-struct overwrite no longer able to lose a remote write.
- **A remote client can drive the physics.** Commanding 8 kW and 7000 Pa over
  OPC-UA against the real binary in headless mode produced BT-12 rising from
  21.00 to 48.28 degC with 0.1225 kg/s of forced circulation, over 61.3 s of
  simulated time. A steady-state energy balance predicts about 38 K against the
  27.3 K measured mid-warm-up — right order, right sign, consistent with a loop
  still heating while the CTAH rejects heat.
- `cargo check --all-targets --target aarch64-linux-android` is clean, and the
  gating was shown to be real rather than vacuous by injecting a type error into
  the physics and confirming it surfaced in the Android check.

**Not** verified, and not to be claimed:

- **Port equivalence between v1 and v2 physics** (`op-wqk.13.6`). The maintainer
  has done validation work on v1's physics; that does **not** transfer to a port.
  Until the comparison is run, v2 is "a faithful port of validated physics", not
  "validated".
- **Termux/Android on-device.** The `aarch64-linux-android` check is a proxy. No
  native on-device build has been run.
- **Both GUIs.** Neither egui window was ever opened — the session had no
  display. Layout is compile-verified only.
- **mDNS discovery in the field.** Never exercised on a real multicast network.
- The thermal-hydraulics being *accurate*. The physics check above verifies the
  control path and that the response is not nonsense. It is not a validation of
  the model.

## Takeaway

The useful generalisation is not "AI wrote it, human checked it". It is that the
human's leverage was concentrated in a small number of **architectural and
scoping decisions** — what to copy, where state lives, what a correctness
criterion should require — while the mechanical verification was done by tools.
The agents' own confidence was the least reliable signal in the whole session.

Per `RESPONSIBLE_USE.md`, everything produced in this session remains
**untrusted draft material until human-reviewed**, and CIET v2 is an **offline
educational demonstration** — not for facility operation, reactor control,
licensing, or safety-critical decisions.
