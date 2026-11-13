# System code ↔ point kinetics coupling interface (gh:#1084)

> Research, education and V&V only. AI draft, unreviewed.

Part of the RELAP-7 parity epic (gh:#1056). RELAP-7 Theory Manual §8, Fig 7
(cited, not quoted). Code: `nee_soon::system_coupling`
(`LoopKineticsCoupling`, `SystemCodeSide`, `PicardControl`, `node_mesh`).
The facade entry is `NeeSoon::new_loop_kinetics_coupling`.

## Design

- **The thermal-hydraulic side.** A system code implements `SystemCodeSide`.
  It reports node fields, takes node powers and advances. The trait is a
  compile-time contract, never used as `dyn`. `tampines` and `tuas` gain no
  dependency on `teh-o-prke` or `outram-foam`.
- **Feedback:** GeN-Foam's `ReactivityFeedback` (reused), evaluated on a
  `CouplingRegion` whose cells are the core nodes. `node_mesh` builds a 1-D
  line mesh for it.
- **Kinetics:** `teh_o_prke` `PointKinetics`, backward Euler.
- **Outer iteration:** a Picard repeat of each step until the relative power
  change is below a tolerance. GeN-Foam's `MultiPhysicsSolver` was not
  reusable here: it dispatches over its own closed `RegionModel` enum, which
  cannot hold a system code without that crate depending on one. With one
  pass, this is loose operator-split coupling.

## Predictions (written before the first run, 2026-11-01)

1. The port's feedback equals the hand `−α (T̄_f − T_ref)` to round-off.
2. Picard to 1e-12 reproduces a monolithic backward-Euler solve of the joint
   13-unknown system (petir JFNK) within 1e-8 at every step.
3. Loose coupling's splitting error halves with Δt.

## Results (`tests/system_coupling.rs`, release)

The loop has three lumped nodes (fuel and coolant temperature each). It runs
six-group U-235 kinetics at 3 MW with Λ = 1e-4 s and α = 3e-5/K, through a
+0.1 β step held for 2 s at Δt = 0.01 s.

| # | Measured | Prediction |
|---|---|---|
| 1 | −3e-4 to round-off; zero at the reference state | held |
| 2 | worst power difference ~~3.8e-12~~ **4.2e-12**, fuel temperature ~~7.3e-15~~ **5.9e-15**; P(2 s) ~~3.4338~~ **3.4585 MW** | held |
| 3 | ~~2.420e-4~~ **2.574e-4** at Δt 0.02 s, ~~1.210e-4~~ **1.286e-4** at 0.01 s; ratio 2.001 | held |

**Re-measured 2026-11-10 (gh:#1319).** `teh-o-prke`'s U-235 six-group
default changed from its textbook set to ENDF/B-VIII.0 (beta 0.0065 to
0.0065230; mean precursor life 13.05 to 11.06 s). Struck values are the
2026-11-01 record; bold values are from `cargo test --release -p nee_soon
--test system_coupling -- --nocapture`. The verification results (coupled
= monolithic, first-order splitting) are unchanged in kind; P(2 s) rose
0.72 %.

## Limits

- **The thermal-hydraulic side is a lumped model.** A tampines `HemNetwork`
  with `heat_structure` rods can implement `SystemCodeSide` the same way, but
  that run is not in the test.
- **Not yet mapped:**
  - to the GeN-Foam multi-region meshes; the RBF map exists in
    `multi_region::rbf_mapping` but is not wired;
  - to decay heat, since `PointReactor` needs ANS data the workspace cannot
    ship.
- **Node fields are bare `f64`** in SI units (K, kg/m³), because they are
  field values written into a GeN-Foam region.
