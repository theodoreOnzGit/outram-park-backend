# sembawang beyond the capstone

> **Extended deep dive.** Read [rung 7](../rungs/07-capstone.md) first.
> **Research, education and V&V only.** Nothing here is a source term, a dose,
> a siting or a licensing argument for HTR-10, NuScale or any plant
> (`RESPONSIBLE_USE.md`).

> **Review status:** AI-assisted first draft, 2026-10-04, not yet reviewed by
> a human. Built from commit
> [`@@COMMIT_SHORT@@`](https://github.com/theodoreOnzGit/outram-park-backend/commit/@@COMMIT@@).

`sembawang` is the source-term crate and the orchestrator of the offsite
chain. What it computes is **release from TRISO fuel under a temperature
history the caller prescribes**, by calling `boon-lay`'s TRISO-ATOPS fork; the
release physics itself belongs to the [TRISO-ATOPS track](../../triso-atops/index.html).
Severe-accident progression (melt, relocation, vessel failure, hydrogen,
aerosols) is in its name and **not implemented**. This page covers the
modules rung 7 uses only in passing.

## The inputs the crate refuses to compute

- **The temperature transient** ([`scenario`](../../../api/sembawang/scenario/index.html)):
  `TemperatureTransient::from_ramp` (an analytic ramp and hold, for shakedown
  and sweeps) or `from_nodes` (any per-node history, so a trace from
  `htgr_sim_v1` drops in), indexed `[ring][time][axial]` as `boon-lay`'s
  fixture is. A run's credibility is the credibility of its transient.
- **The core inventory** ([`inventory`](../../../api/sembawang/inventory/index.html)):
  per radial ring, spread axially by `boon-lay`'s own distribution. Its doc
  explains why not to build one from independent fission yields: Cs-137's
  independent yield is about two orders of magnitude below its cumulative
  yield, so the inventory would come out silently ~100× low.

## The venting seam: a defect made unwriteable

TRISO-ATOPS moves released activity out of the core by **venting** when the
coolant heats. Upstream pairs the selected venting samples (a **gather**)
with a **prefix** of the temperature array. Those agree only when the venting
mask is a contiguous run from index 0; on a heat–cool–reheat transient they do
not, and one node's Xe-133 release was measured to differ by tens of per cent.
[`VentingWindow`](../../../api/sembawang/accident/venting/struct.VentingWindow.html)
owns the indices, and **no method returns a prefix**, so upstream's pairing
cannot be written without visibly slicing an array by hand.

The [`Venting`](../../../api/sembawang/accident/release/enum.Venting.html)
modes: `Upstream` (heat-up venting, with upstream's isothermal branch: a
uniform constant hold vents fully, restored after gh:#446 found the port had
dropped it), `FullFlowThrough` (everything released leaves the core; the
conservative ingress limit) and `Prescribed` (a caller's exchanged fraction,
e.g. `Venting::gao_shi_htr10_cavity_ventilation`).

> **On the [Code Review map](../../../code-review/):** start at [`accident_release_with_venting`](../../../code-review/#crates/sembawang/src/accident/release.rs::accident_release_with_venting). Each card opens the function's source and shows its review stamp.

## Caveats as data

TRISO-ATOPS reproduces upstream behaviours a caller must know about, so
[`error::Caveats`](../../../api/sembawang/error/struct.Caveats.html) returns
them **with the result** rather than logging them. The capstone's
`negative_atom_count_seen = true` (silver and Cs-137) is one: upstream's
release does not clamp a negative increment, the port reports it and floors
only at `changi`'s `SourceTerm` boundary.

## Units: one crossing point

`boon-lay` types activity as `Frequency` (`A = lambda N`), `changi` as
`Radioactivity`. Same dimension, different Rust types, so the conversion lives
in [`units`](../../../api/sembawang/units/index.html) and nowhere else, and a
release travels as an activity attached to a window rather than as a `T^-2`
rate nobody reads as a rate.

## The hand-off to `changi`

[`chain::pad_for_dispersion`](../../../api/sembawang/chain/fn.pad_for_dispersion.html)
pads a source term with zero-release windows so it partitions a dispersion
run from `t = 0` and carries on after the last release; no activity is added,
removed or moved (asserted in `tests/chain_handoff.rs`). The worked example is
`examples/npmhtgr_chain.rs` (NP-MHTGR, a prescribed heat-up, through
`changi::activity` to Bq·s/m³ and Bq/m²). **The capstone does not use this
path:** its dispersion is `buangkok`'s single plume, not `changi`'s puff train
(rung 7 says why).

## HTR-10 DLOFC: PANAMA into TRISO-ATOPS

[`htr10`](../../../api/sembawang/htr10/index.html) joins `boon-lay`'s two
halves on one reactor: PANAMA-I fuel failure supplies the accident-added
failure fraction, TRISO-ATOPS turns it into released activity, driven by Liu
& Cao's Table 1 inventory. The transient is **HTR-Module's** DLOFC history
(1500 °C at 30 h, from the open JRC (V)HTR-Modul volume), used as a stand-in
for HTR-10's, which is not published in the workspace's literature; its
cooldown time constant and late temperature are **estimates, not cited**, and
flagged as such in the constants' names. Examples:
`htr10_dlofc_panama_source_term` (the release) and `htr10_dlofc_dose` (the
same dispersion and dose chain as the capstone, for comparison).

## HTR-10 against an equal-power LWR

[`lwr_comparison`](../../../api/sembawang/lwr_comparison/index.html) puts
published LWR source terms through **the same** dispersion and dose arithmetic
as the HTR-10 arms, so that the difference is the source term alone. Arms are
paired by initiating event and severity (maintainer decision, gh:#464):
design basis (HTR-10 depressurisation, Liu & Cao Table 8, against an RG 1.183
MHA LOCA through a NuScale-sized containment leaking at its sourced
`L_a` = 0.20 %/day), beyond design basis (the capstone's KORA bound against a
NUREG-1465 core melt into an intact containment), and WASH-1400 PWR 8 as
context. Each side is credited only with its reactor-level barrier (TRISO for
the HTR, the containment for the LWR); nothing around the reactor is credited.
In-containment natural deposition is **pending literature** and never
defaulted. Example: `lwr_nureg1465_counterpart`.

[`ap1000_ted`](../../../api/sembawang/ap1000_ted/index.html) holds a
**digitised published curve**, Dadda et al. (2024) Fig. 7, an AP1000
plume-centreline dose from HotSpot, summed over groups in log–log and scaled
by power, as an overlay for `htgr_sim_v1`. It is their model, not ours, and
`MISMATCHES` lists every way it differs.

None of these LWR numbers is re-measured or quoted here: they are not part
of the dispersion lesson's claims.

## Coverage

| Module | Section |
|---|---|
| `scenario`, `inventory` | inputs |
| `accident::{release, venting}` | venting seam; rung 7 |
| `error` | caveats |
| `units` | units |
| `chain` | hand-off |
| `htr10` | DLOFC; rung 7 (geometry, pools, irradiation stand-in) |
| `lwr_comparison`, `ap1000_ted` | LWR comparison |
