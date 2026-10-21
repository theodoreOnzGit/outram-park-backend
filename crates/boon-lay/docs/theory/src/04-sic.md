# Step 4 — Silicon carbide: the barrier that matters most

> **Research, education and V&V only.** Not for reactor operation, licensing, safety decisions or emergency response.

> **Review status: first draft, 2026-10-18, AI-assisted (Claude Sonnet), not yet reviewed by a human.**

*Previous: [Step 3](./03-layers-chemistry.md) tabulated $D$ in every layer.
This page asks why the SiC layer, 35 µm thick, gets the credit.*

## The question

The SiC is one of five layers and by thickness not the largest. **What does it
do that the others do not, and what can take that away?**

## It does two jobs

1. **Diffusion barrier.** Metals (Cs, Sr, Ag) move through SiC far more slowly
   than through the pyrolytic carbon on either side.
2. **Pressure vessel.** SiC is the strong layer. Fission gas and CO build up
   inside, and the SiC holds them. The porous buffer cannot, and the PyC
   layers are not the load-bearing shell in this model. Step 6 gives the
   stress; step 7 shows how PANAMA-I reduces it to a membrane formula in the
   SiC alone.

If the SiC fails, both jobs fail at once. The layers are described this way in
[rung 1](../../tutorials/triso-atops/triso.html).

## How big is the diffusion contrast?

Not as large as the crate once said. An earlier comment claimed SiC's $D$ is
"about $10^6$ times smaller" than PyC's. That compared the *prefactors* $D_1$
(6.3e-8 against 5.5e-14 m²/s for Cs) and left out the exponentials.

~~SiC's $D$ is about $10^6$ times smaller than the pyrolytic carbon around it.~~
**CORRECTED 2026-10-04 (gh:#531), as `interface.rs` records:** with the
activation energies included, the PyC-to-SiC contrast this crate's
coefficients give is **13 to 4100** over Cs, Sr, Ag and Kr at 1000 to 1600 °C.
For Cs it is 120 at 1000 °C and 440 at 1600 °C.

| Nuclide | T (°C) | $D_{\text{PyC}}/D_{\text{SiC}}$ | $p$ (PyC to SiC) |
|---|---|---|---|
| Cs-137 | 1000 | 120 | 8.3e-3 |
| Cs-137 | 1600 | 440 | 2.3e-3 |
| Sr-90 | 1000 | 4080 | 2.5e-4 |
| Ag-110m | 1600 | 74 | 1.3e-2 |
| Kr-85 | 1600 | 13 | 7.4e-2 |

(From the [rung 4 table](../../tutorials/triso-atops/layers.html), run
2026-10-04. The contrast is among the crate's coefficients; it is **not** a
comparison with a measured barrier.)

*What it means for an atom.* The walker crosses an interface with probability
$p = K D_2/(D_1 + K D_2)$, about $D_{\text{SiC}}/D_{\text{PyC}}$ when that
ratio is small. For Cs at 1200 °C the contrast is 415 and $p = 2.4\times10^{-3}$:
the atom must arrive about 400 times before it gets through. That is why a
random walk across the SiC is slow, and why the Booth sphere (step 9) is
tempting.

Note the temperature trend: for Ag and Kr the contrast **shrinks** as the
temperature rises (Ag 468 to 74; Kr 920 to 13 from 1000 to 1600 °C). The
barrier is weakest where an accident is hottest. For Cs and Sr it does not
shrink in this range. `[citation needed: maintainer to supply the measured
silver release through SiC that the Ag trend should be compared with.]`

## Fewest assumptions to simplification, for this barrier

1. **Resolve the layer.** Walk atoms through a 35 µm shell with its real $D$
   on both sides of every interface. Expensive, because of the hundreds of
   arrivals above.
2. **Booth sphere.** Fold the five layers into one sphere with
   $D' = D/a^2$. *Judgement:* you may do this when one layer sets the whole
   release. For Cs in an intact particle, that is the SiC. *Fails:* for
   species the SiC does not hold back (Kr at 1600 °C, contrast 13), or when
   the SiC is breached.
3. **Failure fraction.** Treat the layer as intact or failed with a probability
   (step 7). An intact SiC gives almost no release of metals; a failed one
   gives the release of the unprotected inner layers.

## How SiC fails

### Pressure failure

The gas builds, the stress in the shell grows, and some SiC shells have a
weak flaw. That is step 6 and step 7, and the boon-lay model is
[`fuel_failure`](../../deep-dives/triso-atops/fuel-failure.html).

### Thermal decomposition at high temperature

At very high temperatures SiC decomposes into gaseous Si and solid graphite,
"so the layer stops being a pressure vessel rather than bursting as one" (the
code's own words). boon-lay fuel failure implements the PANAMA-I formulas for
it as a rate constant

$$k = \frac{375}{d_o} \exp\left(-\frac{556000}{R T_m}\right)\ [\text{s}^{-1}],$$

with $d_o$ the initial SiC thickness in metres, $T_m$ the step's mean
temperature, $Q = 556$ kJ/mol, and an action integral $\zeta=\int k dt$
([`decomposition_rate_constant`](../../code-review/#crates/boon-lay/src/fuel_failure/decomposition.rs::decomposition_rate_constant)).
The code says that above roughly 2000 °C this is the dominant mechanism.

Three honest remarks the code itself makes:

- The 375 must carry units of m/s for the formula to balance; the report does
  not say so. This is a reading the maintainer should know about.
- **Nothing here is verified against a plotted or tabulated output of the
  report.** Only internal algebra is checked (for example, $k \propto 1/d_o$,
  and $Q$ reproducing about 3.8 decades in $k$ over 1600 to 2200 °C).
- The report gives two calibrations of the failure law, for loose particles and
  for particles in a sphere; the code finds Fig. 6 of the report can only have
  used the sphere calibration.

### Attack by fission products on the grain boundaries

PANAMA-I Eqs (10b) and (10c) model fission products attacking the SiC grain
boundaries, which widens the scatter in the shell's strength. In the code, this
is **off by default**, and that is the report's own default: grain-boundary
corrosion "is not generally taken into consideration". Figs. 7 and 8 of the
report show it worth one to two decades in release at 1600 °C
([`grain_boundary.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/grain_boundary.rs)).
The repo does not name palladium as the attacking species there.
`[citation needed: maintainer to supply the evidence that Pd is the principal
attacker of SiC.]`

## What we assumed

- The PyC and SiC coefficients are Jiang et al.'s correlations as transcribed;
  the check is against points read from their plotted curves.
- A single temperature for the whole particle and a thin-shell SiC.
- PANAMA-I's decomposition constants are used as printed (with the unit
  reading above); nothing validates them against the report.
- Grain-boundary corrosion is off unless selected.

## Next question

We know the layers, and SiC dominates as both barrier and pressure vessel.
How do we simulate the whole particle? The next part of the book walks down
a fidelity ladder: the high-fidelity finite-element way first
([Step 5](./05a-high-fidelity-fem.md)), then the particle as a pressure
vessel and PANAMA's failure model (Steps 6 and 7), then normal diffusion,
one atom at a time and then FRESCO-style (Steps 8 and 9), and finally what
boon-lay adds (Steps 10 and 11).
