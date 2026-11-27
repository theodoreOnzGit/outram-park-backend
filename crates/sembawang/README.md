# sembawang

**SEMBAWANG** — *Severe-accident Evolution and Melt Behaviour Analysis Workbench
for Advanced Nuclear Geometries*. "Workbench" because it is a toolkit, not a
single code. Letter by letter: **S**evere-accident **E**volution and **M**elt
**B**ehaviour **A**nalysis **W**orkbench for **A**dvanced **N**uclear
**G**eometries.

**The one question it answers:** what gets released in a severe accident,
and how does it reach the rest of the offsite chain?

SEMBAWANG makes **severe-accident source terms studyable and
challengeable**: a reduced, clean-room model of the Fukushima Daiichi BWR
Mark I accident whose equations follow the public MELCOR manuals (no MELCOR
source is held) and whose results are compared with the OECD/NEA BSAF
benchmark bands, including the two it misses; and a TRISO release chain for
HTR-10 in which every input is classified as the reactor's own, derived, a
stand-in or an estimate, so a reader can see which numbers the answer rests
on.

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Research, education and V&V only. Not for nuclear facility operation, reactor control, safety-critical, licensing, emergency planning or emergency-response decisions.

(Banner added 2026-11-27; this README had none, only the "Limits" list
below.)

## Knowledge in this crate

See the workspace [vision](../../VISION.md).

| Knowledge artifact | In this crate |
|---|---|
| Scientific literature | OECD/NEA BSAF (2021), `nea2021bsaf7525`, the Fukushima validation target, and the MELCOR manuals, `sandia2017melcorvol1` (both proprietary tier, catalogued in `crates/kovan-literature/CATALOGUE.md`); IAEA-TECDOC-978 (`iaea1997tecdoc978`, data in `reference/tecdoc978/`); USNRC HRTD ch. 7.1 Table 7.1-2 for the Unit 2 and 3 sequences; Liu & Cao (2002) HTR-10 tables (through `changi`); NUREG-1465, RG 1.183 and WASH-1400 for the LWR comparison (`reference/lwr/`). No upstream code is ported here; the TRISO release physics is `boon-lay`'s TRISO-ATOPS fork. |
| Mathematical equations | Module docs of `src/bwr_mark1/` (core heat-up, Zr-steam oxidation, relocation, lower-head failure, containment network, cavity/MCCI, fission-product decay) and of `src/accident/`, `src/htr10.rs`, `src/chain.rs`, `src/lwr_comparison.rs`. The MELCOR aerosol, vapour and iodine equations moved to `outram_park_fork_offbeat::melcor` (gh:#1335). |
| Source code | `src/`: `bwr_mark1` (plant model, `units23` for Units 2 and 3), `accident` (release, venting), `scenario`, `inventory`, `chain`, `htr10`, `lwr_comparison`, `ap1000_ted`, `units`. |
| Interactive simulations | None in this crate. `crates/outram-park-digital-twin-engine/examples/fukushima_web` consumes `bwr_mark1`. CLI examples in `examples/` (listed under "Running", plus `bwr_mark1_ensemble`, `bwr_mark1_sensitivity`, `lwr_nureg1465_counterpart`, `htr10_dlofc_dose`). |
| Visualisations | Drawn containment-network geometry in `verification_and_validation/bwr_mark1/geometry/` (`examples/containment_network_geometry.rs`). |
| Verification and validation evidence | `verification_and_validation/bwr_mark1/` (containment network, Units 2 and 3 sequence, ensemble, input sensitivity, lower-head wall, `melcor-unit-vv/`) and `verification_and_validation/htr10-dlofc-panama-triso-atops.md`. As recorded: Unit 1 lower head fails at 5.48 h against the BSAF band 7.45–17.16 h, and in-vessel H2 at 25 h is inside its band (2026-11-02; those pins have been ignored since 2026-11-26, gh:#1810, and the numbers were not re-checked after that day's swell fix); the HTR-10 PANAMA seam raises the source term by 0.08 % (2026-09-24); normal-operation pools match upstream TRISO-ATOPS to 3.8e-12. |
| Human reviews and corrections | 0 functions carry kovan human-review stamps. `examples/htr10_air_ingress_human_audited_example.rs` is written for the maintainer to inspect by hand. The comparison pairing was reframed twice by maintainer decision (#450, #464, struck through above); the maintainer ruled no pins on the Units 2 and 3 sequence until the model is checked (gh:#1110). No entry in `docs/human-corrections-to-ai-work.md` names this crate. |

**Status:** an AI draft (kovan maturity 1); the release chain has
code-to-code checks through `boon-lay`, the BWR model is uncalibrated and
outside two BSAF Unit 1 bands, results on the containment network are
tentative until a human checks its drawn geometry, and nothing is validated.

**Scope (maintainer decision 2026-09-21, #235): severe accident and source
term, and orchestrator of the offsite chain.** ~~the source-term end of the
offsite chain (SEMBAWANG -> CHANGI -> REDHILL)~~ SEMBAWANG computes the source
term and drives the rest of the chain:

```text
boon-lay (TRISO release) -> SEMBAWANG source term -> CHANGI (air, ground) -> REDHILL (ground transport)
                            RAFFLES: uncertainty propagation across the whole chain (#238)
```

Wired in today: `boon-lay`, `changi` and, since 2026-09-30, `buangkok`. Not yet:
`raffles` (#238) and `redhill` (a placeholder). ~~**No dose is computed**~~
**UPDATED 2026-09-30:** ~~the library still computes no dose.~~ One example,
`examples/htr10_air_ingress_kora_bound.rs`, computes a research-grade dose at
400 m through `buangkok`'s Gaussian plume. **CORRECTED 2026-09-30 (#452):** the
library module `lwr_comparison` now computes a research-grade dose through
the same `buangkok` chain, comparing HTR-10 with an LWR at equal power
(`examples/lwr_nureg1465_counterpart.rs`). ~~It places the HTR-10 bounding
air-ingress case beside LWR counterparts (NUREG-1465, RG 1.183 per unit leak
rate, WASH-1400 PWR 8).~~ ~~**Reframed 2026-09-30 (maintainer decision, #450):**
the primary comparison is design basis against design basis ... The HTR-10
bounding air-ingress case against WASH-1400 PWR 8 is the secondary,
beyond-design-basis bounding comparison.~~ **Reframed again 2026-09-30
(maintainer decision, #464): paired by initiating event and severity**, DLOFC
(HTR) against LOCA (LWR). *Design basis: DLOFC vs LOCA*: the HTR-10
depressurisation (Liu & Cao Table 8) against the RG 1.183 MHA LOCA at NuScale's
L_a. *Beyond design basis: DLOFC + air ingress (bounding) vs LOCA + core melt*:
the KORA bound against NUREG-1465 Table 3.13 (all phases) in an intact
containment leaking at L_a. WASH-1400 PWR 8 is context only (no melt,
uncontained). Each side is credited only with its reactor-level barrier
(containment vessel; TRISO particles), with no pool, building or filter credit
on either side. None of this is described as PSA. See `docs/ecosystem-naming.md` decision 7.

## What exists, and what does not

**Exists:** the fission-product *release* path for TRISO fuel. A caller-prescribed
temperature transient and core inventory go through `boon-lay`'s TRISO-ATOPS fork
(diffusion, release fractions, SiC breakthrough) and come out as a
`changi::activity::source::SourceTerm`.

~~**Does not exist:** severe-accident *progression* — no melt, relocation, vessel
failure, molten-core-concrete interaction, hydrogen or aerosol physics. The
temperature transient is an **input**; nothing here computes it.~~
**CORRECTED 2026-10-29 (gh:#1033)**: **a reduced progression model exists for one plant.**
(**CORRECTED 2026-11-27**: Units 2 and 3 were added on 2026-11-02, gh:#1109:
`src/bwr_mark1/units23.rs`, `Plant::for_unit`, with RCIC and HPCI in
`turbine_pump.rs`, recorded in
`verification_and_validation/bwr_mark1/units23_sequence.md`; checked by
reading those files.)
`src/bwr_mark1/` is Fukushima Daiichi Unit 1 (BWR/3, Mark I) after the scram:
vessel, core heat-up, Zr-steam oxidation and hydrogen, relocation, a lower-head
failure criterion, drywell, wetwell and reactor building. It was the
`dhoby-ghaut` `fukushima_web` demo's physics and moved here (maintainer:
"Physics libraries go to sembawang"); the demo now consumes it. ~~Lumped,~~ an AI
draft, uncalibrated, and ~~**outside the BSAF Unit 1 bands**: lower head ~2 h
early, in-vessel H2 ~3x low~~ **CORRECTED 2026-11-02** (checked against
`compare::the_uncalibrated_baseline_is_pinned` and
`the_network_baseline_is_pinned`): **outside two BSAF Unit 1 bands**, the
lower head ~2 h early (5.48 h against 7.45-17.16 h) and the building
explosion never happening; in-vessel H2 at 25 h is inside its band
(`bwr_mark1::compare`; `cargo run --release -p
~~dhoby-ghaut~~ outram-park-digital-twin-engine --example fukushima_web -- --compare`,
the example moved there 2026-11-05). **Still does not exist:**
~~MCCI, aerosol physics, in-plant fission-product transport,~~ (**CORRECTED
2026-11-02**: MCCI, sectional aerosol physics and in-plant fission-product
transport exist since 2026-10-29: `cavity`, `rn_volume`, `containment_fp`;
**since 2026-11-11 (gh:#1335)** the MELCOR clean-room *equations*
`aerosol`, `vapour`, `rn_volume`, `iodine` and `wire` live in
`outram_park_fork_offbeat::melcor`, re-exported here under their old paths
for one release, while the plant assembly stays here) a general
(MELCOR-class) progression engine. **The containment is a control-volume
network since 2026-11-02** (gh:#1099, maintainer: "switch it"):
`containment_network` on `tampines`' `CvNetwork` (drywell, torus and
building as control volumes, IF97 water, a layered torus pool, inertial and
choked paths), the old lumped containment kept as the explicit ablation
`Mechanisms::without_network_containment()`. Results on the network are
**tentative** until ~~its geometry is drawn~~ a human checks its drawn
geometry (gh:#1110, drawn 2026-11-02 in
`verification_and_validation/bwr_mark1/geometry/`), and the vent's
depressurisation depends on the torus pool's layer count (gh:#1108); what
moved is in `verification_and_validation/bwr_mark1/containment_network.md`
("gh:#1099"). The TRISO release's temperature transient
is still an **input**; `bwr_mark1` does not feed it. Regression fixtures:
`verification_and_validation/bwr_mark1/headless_30h.csv` (the network plant)
and `headless_30h_lumped.csv` (the ablation). (**CORRECTED 2026-11-27**, from
`AGENTS.md`: neither fixture is asserted at present. The network fixture is
stale on purpose since 2026-11-02 and its test ignored; the lumped fixture
was re-blessed after the swell fix and its test ignored on 2026-11-26,
gh:#1810, maintainer: "Ignore sembawang pins for now, nothing to pin
against".) Nor is the
core inventory computed (see `src/inventory.rs` for why `fission-yields-data`
must not be used to build one).

**Also exists (2026-09-24): the HTR-10 case, and the PANAMA seam.** `src/htr10.rs`
joins `boon-lay`'s two halves on one reactor for the first time: PANAMA-I
(`boon_lay::fuel_failure`) supplies TRISO-ATOPS's **accident-added** failure
fraction `f_inc_acc`, and the release is driven by the *published* HTR-10
equilibrium-core inventory from `changi`. Methodology, every input classified
(HTR-10's own / derived / stand-in / **estimate**), the ablation control and the
grid-convergence study are in
[`verification_and_validation/htr10-dlofc-panama-triso-atops.md`](verification_and_validation/htr10-dlofc-panama-triso-atops.md).

The headline preliminary result, and it is a negative one: at the HTR-Module
DLOFC peak of 1500 °C the accident-added failure fraction is `1.07e-7` against an
as-manufactured `1.23e-4`, so the seam raises the source term by **0.08 %**. An
HTR-10 DLOFC source term at that peak is governed by as-manufactured fuel
quality, not by accident-induced particle failure; the crossover is above the
1600 °C design limit. **Not a source term for HTR-10** — the transient is
HTR-Module's, the fuel-quality fractions are NP-MHTGR's, and the whole core is
run on the hot-node history. GitHub #296.

**Also exists (2026-09-21): the join to `changi`.** `sembawang::chain::pad_for_dispersion`
lines the release windows up with a `changi` dispersion run, so one run goes from
the prescribed transient to Bq·s/m³ in air and Bq/m² on the ground. ~~No dose is
computed.~~ **CORRECTED 2026-11-27** — the join itself computes no dose, but
the crate does: `lwr_comparison` computes a research-grade dose through
`buangkok` (#452), as stated at the top of this README.

## `no_std` + `alloc` (gh:#1641)

The default `std` feature can be turned off: the library is then `no_std` +
`alloc` and builds for bare metal
(`cargo check --release -p sembawang --lib --no-default-features --target x86_64-unknown-none`,
gated by `scripts/check-nostd.sh`). The BWR Mark I plant model, the release
chain, the HTR-10 cases and the LWR comparison build without it. What `std`
carries: ~~the fission-product decay summation that loads the OpenMC decay
libraries (`bwr_mark1::fp_decay::compute`; its committed element tables build
without std),~~ printing, the integration tests and the examples. **CHANGED
2026-11-22:** `fp_decay::compute` builds without `std` too, since the OpenMC
decay libraries (1.1.0) are `no_std`.

## Running

```bash
cargo run --release -p sembawang --example npmhtgr_release   # release only: Bq, Ci, fraction per nuclide
cargo run --release -p sembawang --example npmhtgr_chain     # release -> changi: Bq, Ci, Bq.s/m3, Bq/m2 vs distance
cargo run --release -p sembawang --example htr10_dlofc_panama_source_term   # HTR-10 DLOFC: PANAMA -> TRISO-ATOPS
cargo test --release -p sembawang --lib --tests
```

`npmhtgr_chain` records its own time-step convergence study in its doc comment.
From 500 m out the results converge at a 10 s step. **The 100 m and 200 m rows
do not converge** and should not be quoted. `tests/chain_handoff.rs` checks the
join: padding conserves activity, the chain is linear in inventory, and noble
gases deposit exactly zero. These are consistency checks, not verification.

## Limits, stated up front

- Research, education and V&V only. Not for emergency planning or response,
  licensing, or any safety-critical decision.
- The orchestration has **no upstream** and no code-to-code verification. The
  release physics underneath is verified against upstream TRISO-ATOPS in
  `boon-lay`.
- Known upstream behaviours are returned as data (`Caveats`), not logged.
- The three accident-phase failure fractions are required arguments, not
  defaults: they are not in the cited reference case.
- ~~Starting from empty normal-operation pools under-predicts the early release.~~ **CORRECTED 2026-09-30 (#448):** the release now starts from real normal-operation pools by default (code-to-code verified against upstream TRISO-ATOPS at 3.8e-12). Empty pools remain as an explicit ablation, `PlantParameters::without_normal_operation_pools`, and their error is nuclide-dependent: in upstream, metals are **over**-predicted (Ag-110m ×2, Cs-137 ×1.6) and un-scrubbed noble gases slightly under-predicted.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.
