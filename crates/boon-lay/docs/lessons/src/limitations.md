# Limitations and deliberate omissions

A model is only as useful as a clear statement of what it leaves out. This
page collects the omissions stated in the code and docs. Each one links to
where it is stated.

## Not validated

Nothing in `boon-lay` is validated against measured fission-product release
or particle-failure data. The workspace's literature has no measured HTR-10
failure, free-uranium or release fraction for the fuel-failure model to be
compared with
([`htr10/mod.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/htr10/mod.rs#L126-L137)).
Nothing here is for licensing, safety decisions or emergency planning. The
accident module repeats that warning, because an accident source term is the
number most likely to be misused
([`accident/mod.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/triso_atops_fork/accident/mod.rs)).

## boon-lay fuel failure

- **As-manufactured defects `φ_o` are an input**, not modelled. The report's
  "target" value `6·10⁻⁵` is offered as a named constant, never as a default
  ([`AS_MANUFACTURED_TARGET`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/mod.rs#L221-L228)).
- **Thin shell only.** The report has no thick-wall form.
- **Built for German TRISO over 1600–2500 °C.** Use anywhere else, HTR-10
  included, is an extrapolation.
- **Not for normal-operation `f_inc`.** It models accident failure.
- **Thermal decomposition is unverified** against any printed result.
- **Grain-boundary corrosion is off by default**, as in the report. It is an
  explicit enum choice at the call site.

## TRISO-ATOPS port

- **It reproduces upstream, including upstream's choices.** Examples are the
  5000-term Booth series and its `1.216·10⁻⁴` floor (gh:#385), and the 84-nuclide
  table with its own half-lives. Where an upstream defect is known, the
  divergence is selectable and named.
- **Depressurisation only.** No air- or water-ingress transport, and no
  building, dust or helium-purification term in the accident release. Flow-
  through options live in `sembawang` and are labelled as not upstream
  (gh:#446).
- **No GUI, and no CLI shell.** Upstream's Tkinter GUI, logging set-up, error
  counter and `argparse` `main` are deliberately not ported. The library calls
  cover what they composed
  ([`docs/triso-atops-fork.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/docs/triso-atops-fork.md)).
- **The port's own `RunFile` cannot read a GUI-written file.** A separate
  reader, `run_file::upstream`, does, and is verified against upstream's
  `process_run_file` (gh:#449).

## Lagrangian engine

- **Single-layer benchmark only.** A full multilayer CRP-6 release record has
  not been written.
- **Transmutation is a framework.** There is one explicit `(n,γ)` channel.
  Per-nuclide cross sections, `(n,2n)`, fission and fission yields are not
  wired in.
- **Placement approximation.** When a decay event cuts a hop short, the atom
  changes identity at the hop's *start*. Timing is exact; position is
  conservative.
- **The old Gaussian-step code is still in the crate**, beside the
  Walk-on-Spheres engine. The crate calls the new engine "the intended
  replacement for the diffusion core"
  ([`first_passage/mod.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/lagrangian_decay_simulator/lagrangian_diffusion/first_passage/mod.rs#L33-L35)),
  and the old code is the case study in [Getting out](./release.md).

## Added 2026-10-04 by the core-lesson pass (gh:#531)

- **No oxidation-failure model.** Measured KORA failures in air are added by
  hand downstream (`sembawang`); the mechanistic attack chain is gh:#441,
  with SiC oxidation gh:#443 and exposed-kernel air oxidation gh:#444
  ([rung 6](../../tutorials/triso-atops/chemistry.html)).
- **Chemistry is three transcribed correlations**, each used outside its
  fitted range in an accident; the hydrolysis fit is clamped to the whole
  inventory at water-ingress pressures (gh:#418).
- **The random walk's `D(T)` covers Ag, Cs, Sr and Kr**; every other element
  diffuses as silver, and the graphite materials are `todo!()` (gh:#541).
- **The decay sampler can panic** on a branch with no daughter
  (spontaneous fission) or on branching ratios summing below 1 (gh:#538).
- **The decay data and TRISO-ATOPS's half-lives come from different
  sources** (ENDF/B-VIII.0 via OpenMC's chain, and the IAEA Live Chart).
- **Accident release is venting-only**: an isothermal hold releases exactly
  0 Bq (gh:#446; [rung 7](../../tutorials/triso-atops/coolant.html)).
- **The GPU path is unverified**, and the CPU path is the reference
  ([compute backends](./compute-backends.md)).

## The examples

The three `egui` examples have no `--headless` mode yet, which the workspace
requires of every GUI simulator. Until they do, they are demonstrations.
