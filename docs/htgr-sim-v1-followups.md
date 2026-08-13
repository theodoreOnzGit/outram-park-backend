# htgr_sim_v1 — requested follow-ups (logged 2026-08-14)

Logged outside active working hours (00:38 Friday, Asia/Singapore) per the
working-hours guardrail in the root `CLAUDE.md`. **Nothing here has been
implemented** — this file only records the request so it can be picked up in
the next active window (Friday 07:30–20:00).

All five items are in
`crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/`.

## Physics

1. **Thermal conduction — make it more realistic.**
   Currently the core side is a single lumped helium node with a first-order
   thermal-inertia relaxation (`CORE_THERMAL_TIME_CONSTANT_S`) and no
   fuel→coolant conduction path at all; fuel temperature comes from the
   kinetics layer independently of the coolant.
   Files: `physics/primary_loop.rs`, `physics/kinetics.rs`.

2. **Fuel temperature feedback.**
   Wire the fuel temperature into the reactivity path so the loop closes
   (currently the Nordheim-Fuchs layer carries its own adiabatic fuel
   feedback, but it is not coupled to the coolant-side heat removal).
   Files: `physics/kinetics.rs`, `physics/mod.rs`.

3. **Increase controller gain — both proportional and integral — to stabilise
   better.**
   Note: the feedwater controller as it stands is **proportional only**, with a
   first-order lag (`FEED_CONTROL_TIME_CONSTANT_S = 10.0 s`); there is no
   integral term to raise yet. Adding one is part of this item.
   File: `physics/secondary_loop.rs`.

## Schematic artwork

4. **Rankine-cycle pump — pipe looks like it is floating.**
   The feed-pump connector runs do not meet the pump glyph. `PumpVisual` draws
   a filled circle of radius `screen_vector.length()/2` centred on
   `screen_position`; the connector endpoints in `app/schematic.rs` were
   positioned against the old painter lines and were not re-anchored to the
   circle edge.
   Files: `app/schematic.rs`, `../../src/components/pump.rs`.

5. **Turbine pipe should connect to the turbine edge, not the shaft.**
   The SG→turbine and turbine→condenser runs currently terminate at the
   turbine's centre line rather than its casing edge.
   Files: `app/schematic.rs`, `../../src/components/turbine.rs`.

## Also queued

- **Merge `origin/develop`** — the branch is currently **412 commits behind**.
  Deliberately not attempted tonight: a merge that large is likely to need
  real conflict resolution, which is substantive work.
- Uncommitted at time of logging: a one-line fix to
  `docs/workspace-maintenance.md:150` (`fhr_sim_v2` example is run with
  `-p outram-park-digital-twin-engine`, not `-p tampines-steam-tables`).

## Note on tracking

Not filed in beads: the beads DB synced from `refs/dolt/data` in this session
does not contain the `op-wqk` epic (nor `op-dt3`/`op-4wv`) and carries
pre-rename crate names, so it does not appear to be the maintainer's live
tracker. Filing there risked ID collisions. Please re-file these as beads from
wherever the current DB lives.
