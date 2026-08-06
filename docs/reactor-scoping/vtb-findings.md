# What the Virtual Test Bed gives us

Findings from a survey of the vendored NRIC/INL Virtual Test Bed
(`reference-data/virtual_test_bed/`, CC-BY-4.0), read against the gaps recorded
in the reactor scoping documents in this directory.

Surveyed 2026-08-06 against upstream branch `devel`. Every number below was read
out of a file; paths are given so each can be re-checked.

> **Attribution.** VTB is CC-BY-4.0 and asks to be cited — see
> `reference-data/virtual_test_bed/NOTICE` and upstream's
> `doc/content/vtb_pages/citing.md`. Property correlations inside VTB carry
> their **own** upstream citations, which must be traced to the original
> reports rather than inherited through VTB, per
> `RESEARCH_INTEGRITY_AND_PROVENANCE.md`.

## 1. The enabling finding: the data is usable without MOOSE

Almost every VTB case needs a MOOSE-family code (Griffin, Pronghorn, SAM,
BISON), most of them access-gated. That does **not** block us, because the
*data* is extractable without running anything:

- **Gold files are readable directly.** Some are CSV. The rest are Exodus II,
  which is plain netCDF-3 — a short classic-netCDF header reader pulls out
  global and nodal variables. No MOOSE and no netCDF library required.
- **Git-LFS objects are anonymously retrievable** by POSTing to the repository's
  LFS batch endpoint. No authentication, no clone.

So cross sections, gold eigenvalues, correlations, geometry and material
properties are all in reach. What is *not* in reach is re-running the cases.

### A caveat that matters more than it sounds

**Gold does not mean published.** Many golds are deliberately truncated so CI
stays fast — `fixed_point_max_its = 1`, `num_steps = 1..5`. Examples found:
the PBMR-400 `ss0` eigenvalue is one iteration, not converged; the gFHR gold
k differs from its own published value in the third decimal; and the MSRE
multiphysics eigenvalue is generated with the precursor transfers disabled, so
it is a **zero-precursor** result.

**Always read the case's `tests` file before treating a gold as an answer.**

## 2. MSRE

Relates to [msre.md](msre.md).

### The validation case

`msr/msre/reactivity_insertion/` is the only MSRE case tagged
`V_and_V:validation` — the rest are `demonstration`. It is a circulating-fuel
point-kinetics model of the **5 MW reactivity-insertion experiment**, and its
documentation figures plot **measured experimental data** against code
predictions, with L2 errors quoted for three coupling levels.

Because the model is fully specified in-repo, this is a digitise-and-compare
target that needs no MOOSE.

### Precursor drift — and the measurement MSRE itself does not provide

`msr/msre/multiphysics_core_model/steady_state/th.i` solves six precursor
scalars over the whole closed loop with upwind advection, turbulent diffusion
and decay. Structurally this is the same equation as
`crates/outram-park-fork-moltres/src/precursors.rs`, with **one term we do not
have**: a turbulent-diffusion contribution.

**There is no MSRE pump-on/pump-off case in VTB.** But the equivalent
measurement exists in the CNRS benchmark, isolated by construction — see §4.

### Fuel-salt correlations — closes a MISSING item

From `doc/content/msr/msre/msre_description.md`, for LiF-BeF2-ZrF4-UF4:

| Property | Correlation |
|---|---|
| Melting point | 722.15 K |
| Density | $\rho = 2553.3 - 0.562\,T$ kg/m³ |
| Viscosity | $\mu = 8.4 \times 10^{-5} \exp(4340/T)$ Pa·s |
| Conductivity | 1.0 W/m·K |
| Heat capacity | 2009.66 J/kg·K |

Coolant salt LiF-BeF2 (66-34): melting 728 K, $\rho = 2146.3 - 0.488\,T$,
$\mu = 1.16\times10^{-4}\exp(3755/T)$, k = 1.1, c_p = 2390.
Hastelloy N: 8860 / 23.6 / 578.

**These four fuel-salt correlations are exactly the
`LiquidMaterial::CustomLiquid` payload [msre.md](msre.md) lists as missing** —
temperature-dependent functions, not the single-point values the openmsr
parameter files give. A 46-point tabulated viscosity alternative (750–1200 K)
sits in `msr/msre/steady_state/msre_loop_1d.i`.

### Kinetics — three mutually inconsistent sets

Flagged because a choice must be made and cited:

| Source | Sum of beta |
|---|---|
| `reactivity_insertion/msre_pke_ss.i` | 2.640e-3 |
| `multiphysics_core_model/steady_state/th.i` | 3.021e-3 |
| `mgxs/xs.xml` (U-235 delayed fractions) | 6.525e-3 |

Only the third is U-235-like. Do not average them; pick one and say which.

### Report identifiers — closes the citation gap

`doc/content/bib/vtb.bib` supplies the identifiers [msre.md](msre.md)
deliberately left unasserted. Now confirmed to exist as citable references:

- **ORNL-TM-728** — design and operations, Part I *(already ingested in full at
  `crates/kovan-literature/open/reports/msre-design-and-operation.json`)*
- **ORNL-TM-732** — Part V, reactor safety analysis
- **ORNL-TM-2316** — physical properties of fuel, coolant and flush salts; the
  source of the correlations above
- **ORNL-TM-2997** — experimental dynamic analysis with U-233 fuel; **this is
  the dynamics/frequency-response source [msre.md](msre.md) predicted**
- **ORNL-TM-3039**, **ORNL-TM-3229**, **ORNL-3626**
- Fratoni et al. (2020) MSRE benchmark evaluation

These were read from a bibliography, not from the reports themselves — obtain
each before citing it in a V&V case.

### Modelling shortcuts to be aware of

VTB's own MSRE decks take shortcuts we should not inherit: the multiphysics case
uses a *coolant*-salt property set for the **fuel** salt; steel conductivity is
applied to the graphite core, and no graphite properties appear in the MSRE
decks at all; and the primary heat exchanger is a prescribed-ambient sink —
structurally the same limitation [msre.md](msre.md) already flags in moltres.

## 3. HTR-10

Relates to [htr10.md](htr10.md).

### Reference eigenvalues, extracted directly

`htgr/htr10/` is Griffin neutronics with SPH equivalence. Read out of the gold
Exodus files without running anything:

| Case | Eigenvalue |
|---|---|
| Initial critical | 1.0009032234669475 |
| Full core, all rods out | 1.1234734552111967 |

Both match the published table to the quoted digits. The golds also carry the
full spatial solution — ten-group scalar fluxes and reaction rates on 23,046
nodes — and the ten-group cross-section library covers all six benchmark states
in one retrievable file.

The documentation carries the **inter-code comparison table** for initial
criticality (MCNP, two Serpent evaluations, Griffin) and for the full core at
three temperatures across seven codes from five countries, plus control-rod
worths with and without SPH correction.

So HTR-10 is a **complete, self-contained neutronics benchmark**: library, mesh
and gold solution, all retrievable and parseable.

### The gap that remains

**VTB has no HTR-10 thermal hydraulics at all** — no loss-of-forced-cooling, no
packed-bed TH, no safety-demonstration transient. For those, the nearest cases
are PBMR-400, HTR-PM or GPBR-200.

### PBMR-400 — the pebble-bed coupled transient

`htgr/pbmr400/` implements the OECD/NEA benchmark's pressurised loss-of-forced-
cooling. The converged coupled steady state is citable from the transient gold
at t = 0 (restored from a checkpoint): average fuel 1068.54 K, maximum fuel
1247.08 K, fission power 374.296 MW, decay heat 25.704 MW.

The transient reference itself is in the documentation **figures**, which plot
average fuel and moderator temperature over 50 hours against five codes — a
digitisable multi-code spread rising to a peak near 1345 K. The steady golds are
truncated CI runs and should not be cited.

## 4. The precursor-drift verification target

**This is the single most directly useful number in the survey for
`outram-park-fork-moltres`.**

`msr/cnrs/` stages a molten-salt cavity benchmark so each step adds exactly one
physics. Two of the gold eigenvalues differ **only** by whether precursors drift:

| Step | Configuration | Eigenvalue |
|---|---|---|
| s02 | neutronics + flow, **no precursor drift** | 1.0046787335957 |
| s11 | **+ precursor drift**, isothermal | 1.0040719026395 |

The difference isolates the drift reactivity worth by construction:
**about 60 pcm, or roughly 0.09 $** using the delayed fraction summed from that
case's own cross-section library.

Our moltres test `flow_reduces_reactivity_monotonically` currently checks this
effect only *qualitatively* — monotone and bounded by beta. Here is the same
quantity with a number attached, on a fully specified geometry with a small,
plain-text, readable cross-section library. That makes it a genuine quantitative
verification target rather than a sanity check.

## 5. Packed-bed closures — closes two MISSING items

[htr10.md](htr10.md) records that packed-bed friction is a `todo!()` and that
effective bed conductivity is "literally zero code in the workspace". VTB
supplies both.

### Friction

Every pebble-bed case in VTB uses **KTA**, not Ergun — the Ergun drag model
appears in no case. The KTA correlation is written out algebraically once, in
`doc/content/htgr/generic-pbr-tutorial/step2.md`:

$$-\frac{dp}{dx} = \psi \frac{1-\varepsilon}{\varepsilon^{3}} \frac{1}{2 D_h \rho} \left(\frac{\dot{m}}{A}\right)^{2}$$

$$\psi = \frac{320}{Re/(1-\varepsilon)} + \frac{6}{\left(Re/(1-\varepsilon)\right)^{0.1}}$$

with a fully worked example **and a checked-in gold pressure drop** to verify
against. That page alone is enough to implement the closure from scratch and
confirm it.

### Effective conductivity

`htgr/generic-pbr/pbr.i` carries precomputed Zehner-Bauer-Schlünder values as an
18-point tabulation of pebble-bed effective conductivity from 300 to 2000 K
(11.94 to 44.95 W/m·K), plus gap heat-transfer coefficients and a ten-point
effective TRISO-compact conductivity. Directly usable as a reference to check an
implementation against.

`pbfhr/gFHR/data/gFHR_porosity.txt` is the only real two-dimensional porosity
map in the repository — 16 radial by 40 axial nodes showing the near-wall
porosity rise. Every other case uses a scalar.

### Graphite properties — closes a third MISSING item

The workspace has no graphite at all. VTB has several sets: a constant
1780 / 1697 / 26 recurring across cases; temperature **and fast-neutron-fluence**
dependent correlations for UO2, buffer, PyC, SiC and matrix in the HTR-PM pebble
model; conventional grade tables in the MHTGR and HTTF decks; and IG-110 fits
with an **anisotropic conduction tensor** for HTTR.

## 6. Recommended use

1. **HTR-10 neutronics** is the cheapest real validation win available — a
   complete benchmark, extractable today, against a published multi-code table.
2. **The CNRS precursor-drift worth** turns a qualitative moltres test into a
   quantitative one.
3. **KTA friction and ZBS conductivity** close the two closure gaps that block
   any credible pebble-bed thermal-hydraulics work, and both come with numbers
   to verify against.
4. **MSRE salt correlations** fill the `CustomLiquid` payload.
5. **The MSRE reactivity-insertion figures** are the validation-against-
   measurement target, and require figure digitisation — which must itself be
   documented as a processing step.

## 7. Caveats carried forward

- Check every gold against its `tests` file before citing it.
- VTB's V&V tags are honest and worth reading: `validation` versus
  `verification` versus `demonstration` mean different things here.
- Several suspected deck bugs were noted in passing and not verified against
  upstream intent; they are not repeated here as fact.
- Nothing in this document has been executed or reproduced by us. It records
  what the vendored material contains, not results we have obtained.
