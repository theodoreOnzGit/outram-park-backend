# Step 5 — The high-fidelity way: fuel performance by FEM, as BISON does in MOOSE

> **Research, education and V&V only.** Not for reactor operation, licensing, safety decisions or emergency response.
>
> **Review status: AI-assisted first draft (2026-10-21), not yet reviewed by a human.**

**Back:** [Step 4, silicon carbide](./04-sic.md).
**The question that led here:** steps 1 to 4 followed an atom from its birth
to the SiC, and showed that the SiC is both the main barrier and the
pressure vessel. *If we wanted to simulate the whole particle with the fewest
assumptions, what would we have to solve?*

**This page answers:** what a high-fidelity TRISO fuel performance code
resolves, what this workspace has of it (nothing yet), and what question that
leaves for the next rung down.

This page has **no Code Review link**, because no function in this workspace
implements it. That is the honest state, not an omission.

---

## 1. Why the particle needs a mechanics model at all

The layers do not just sit there. Each one has a job, and several of the jobs
are mechanical. The JRC report on the (V)HTR-Modul lists them (Kugeler,
Nabielek and Buckthorpe 2017, Table 5, pp. 32–33):

- the **buffer** "provides void volume for gaseous fission products and
  reaction products (CO, CO2)", and "accommodates fuel-kernel swelling";
- the **inner and outer PyC** each "induces compressive stresses in SiC due
  to irradiation induced shrinkage";
- the **SiC** is the "load bearing layer for particle", and the primary
  barrier to fission products.

So the stress in the SiC is a **competition in time**: the PyC layers shrink
under irradiation and squeeze the SiC, while gas builds up in the buffer and
pushes it outwards. Whether the SiC breaks depends on which one wins, and when.

**Predict.** Early in irradiation, is the SiC in tension or in compression?
And late in irradiation, at high burnup?

## 2. What a BISON-style simulation resolves

BISON is Idaho National Laboratory's finite-element fuel performance code,
built on the MOOSE framework. Its TRISO capability is described by Hales et
al. (2013), who give the governing equations and material models for particle
fuel, verify against an IAEA benchmark, and analyse particles of increasing
geometric complexity in 1D, 2D axisymmetric and 3D (from the public OSTI
abstract, record 1111031; the paper's full text was not read). A corpus source
corroborates that BISON is a TRISO fuel performance code run on the IAEA
benchmark. PNNL-31427 lists BISON among the models that include pressure-vessel
failure, and IPyC cracking driven by PyC dimensional change and creep (Wells,
Phillips and Geelhood 2021, pp. 4.9 and 4.12). It also reports that Hales et
al. (2020) used BISON on CRP-6 Case 12 and compared its tangential SiC stress
against STRESS3, ATLAS and PARFUME (p. 4.5). Jiang et al. (2021) describe TRISO
fuel performance and failure analysis with BISON. That paper's text was not
read here, but the maintainer's dissertation (Ong 2024, pp. 325 to 326, open
corpus) takes its UCO, SiC and pyrolytic-carbon heat-capacity correlations and
lists it under the same title, so the paper does carry BISON's TRISO material
data.

A simulation of this kind resolves, on a mesh of the particle and over the
whole irradiation history:

1. **Heat conduction** through kernel, buffer, PyC and SiC, giving the
   temperature in every layer.
2. **Stress equilibrium** in every layer, with the strain split into parts:

   $$\varepsilon = \varepsilon_{el} + \varepsilon_{th} + \varepsilon_{irr} + \varepsilon_{cr}$$

   that is, elastic, thermal, irradiation-induced dimensional change (PyC
   shrinkage, SiC swelling, kernel swelling, buffer densification) and
   irradiation creep. This is the generic form of a small-strain fuel
   mechanics model, written here for orientation. BISON's own constitutive
   laws are in Hales et al. (2013), which this page cites but does not
   reproduce.
3. **Gas pressure** in the buffer from fission gas and CO, which loads the
   inner surface of the IPyC.
4. **Interfaces** between layers, which may stay bonded or come apart.
5. **A failure probability** for the SiC (and the PyC) from Weibull
   statistics.
6. **Multidimensional effects**: shapes and defects that break spherical
   symmetry.

### A worked example from the literature

Schappel, Pastore and Terrani (2021) ran BISON on the IAEA benchmark case 11
particle (section 3.2 of the accepted manuscript): 351 days at a
volume-averaged temperature of 1608 K, a fast fluence of
$7.2 \times 10^{25}$ n/m², and 14 % FIMA. The benchmark's own input table
agrees: Case 11 is the HFR-P4/3 irradiation, 351 efpd, 14 % FIMA,
$7.2 \times 10^{25}$ n/m² ($E > 0.18$ MeV), with a time- and volume-averaged
irradiation temperature of 1335 °C, which is 1608 K (IAEA 2012, section 9.3.1.3
and Table 9.12, pp. 518 to 519). In their words: "Initially, the SiC
shell is placed in compression by the shrinkage of the OPyC and IPyC layers.
However, as the shrinkage rate slows, the PyC creep relaxes the stress. Then
as the gas pressure increases, the stresses become increasingly tensile." The
predicted gas pressure was about 67 MPa at the end of the simulation.

These are their numbers, quoted as published. The gas pressure was not checked
against the benchmark's plotted curves (IAEA 2012, Fig. 9.19), which this page
did not read off. **Nothing in this workspace has reproduced them.**

So the answer to the prediction above is compression early, then tension
late. A pressure-vessel model that keeps only the gas (step 6) sees only the
second half of that story.

### Weibull failure, element by element

A ceramic breaks at its weakest flaw, so a bigger volume under the same
stress is more likely to break. The standard weakest-link (Weibull) form for
the failure probability of a volume $V$ under a uniform stress $\sigma$ is

$$P_f = 1 - \exp\left[-\frac{V}{V_c}\left(\frac{\sigma}{\sigma_c}\right)^{m}\right]$$

with $\sigma_c$ the characteristic strength measured on samples of volume
$V_c$, and $m$ the Weibull modulus. Schappel, Pastore and Terrani (2021,
section 2.1) apply Weibull statistics per mesh element, and note that
normalising the element volume by $V_c$ "is critical in the correct
application of Weibull statistics because crack nucleation occurs at flaws in
the ceramic materials". The PANAMA-I formulas of [step 7](./09-panama.md) use
the same Weibull idea, applied once to a thin SiC shell rather than element
by element.

### Why one dimension is not enough

Spherical symmetry makes a TRISO particle a 1D problem, and 1D is cheap. Some
of what breaks particles is not symmetric.

- **Debonding and cracked layers.** Schappel, Pastore and Terrani (2021,
  section 1) summarise the AGR-1 post-irradiation observations: debonding
  between the buffer and the IPyC, "resulting from irradiation-induced buffer
  densification"; and IPyC–SiC debonding, which "was determined to adversely
  affect the propensity to cracking of the SiC layer", with radial cracks
  through the whole SiC thickness in some particles. A crack or a gap at one
  place on the sphere concentrates stress there, which needs at least a 2D
  mesh. Two corpus sources agree on debonding. PNNL-31427 (section 4.4, p.
  4.12) states that debonding occurs when the radial stress from IPyC shrinkage
  exceeds the bond strength between the layers, and that it "can lead to larger
  tensile stresses in the SiC". IAEA-TECDOC-1674 (section 8.3.2.6, p. 454)
  reports sections of HFR-K6/3 particles with buffer–IPyC and IPyC–SiC
  debonding, and radial SiC cracks in two of four.
- **Aspherical particles.** A particle with a flattened facet is expected to
  carry a higher local SiC stress than a sphere under the same pressure.
  ~~[citation needed]~~ **Partly sourced 2026-10-21, from the maintainer's
  proprietary folder only** (no open-corpus source describes it). NUREG/CR-6844
  (p. 2-7) lists non-spherical shapes, with uneven and thin coatings, among
  factors that would reduce the internal pressure the SiC survives
  "considerably" below its idealised scoping calculation. The same report says
  some out-of-roundness can be tolerated and that faceted particles "have
  performed well" in irradiation (p. 2-31 and Fig. 2-19, p. 2-32), so
  asphericity erodes margin and is not a failure by itself. IAEA-TECDOC-1674
  (section 9.1.2.8, p. 485) says PARFUME takes the effect of aspherical
  geometry from finite-element analysis of aspherical particles combined with
  the 1D closed-form solution, and its code table (Table 9.1, p. 489) credits
  only one of the five codes in Part I, ATLAS, with modelling faceted
  particles. The analysis both reports point to is Miller and Wadsworth
  (1994), *Journal of Nuclear Materials* 211, 57–69, listed in NUREG/CR-6844's
  references (not in the corpus, not read). No source here gives a stress
  magnitude for a given facet.

## 3. What this workspace has

- **No TRISO fuel performance model.** There is no BISON-style code here and
  no port of one. A clean reimplementation is planned for 2027, tracked as
  [gh:#1011](https://github.com/theodoreOnzGit/outram-park-backend/issues/1011).
- **BISON itself is not open source** (maintainer's statement, 2026-10-21),
  so it can be neither ported nor vendored here. ~~[citation needed]~~
  **Checked 2026-10-21, still unsourced:** none of the corpus texts that
  mention BISON (PNNL-31427, the TRISO-ATOPS source-term paper, Ong 2024)
  states its licence or distribution terms, so this page cites nothing for
  them and leaves the statement attributed to the maintainer.
- **Some building blocks.** The workspace's finite-element crate
  `farrer-park` has 2D plane and 3D solid elements, but no TRISO model, no
  layered irradiation-creep or swelling laws, and no interface debonding; see
  [step 6](./08-pressure-stress.md), which lists what a first-principles
  particle model would need.

So every particle-failure number on the following pages comes from a cheaper
model, and none of those models has been compared against a finite-element
particle in this workspace.

## Why each simplification is allowed

**Kinds of justification used below.** (a) negligible effect, with an estimate; (b) computationally too expensive to be practical; (c) bounding or conservative, with the side it errs on; (d) data not available. **Open** means this page cannot justify it.

| What the following steps drop | Kind | Justification |
|---|---|---|
| The whole finite-element particle model | (d), and (b) | No model exists here (gh:#1011), and BISON is not open. A per-particle solve for every particle in a core is also costly (about $2 \times 10^{8}$ particles for HTR-10; step 6). What dropping it costs is **not measured** |
| PyC shrinkage and creep, which put the SiC in compression early | (d) | The laws are not in the repo. Dropping a compressive term should err towards *more* SiC tension, so towards (c) high failure, but this is **not quantified** |
| Debonding, cracked layers and asphericity | (d) | They need a 2D or 3D mechanics model. **Open** |

## What we assumed

- The layer functions are as the JRC report states them for the German
  reference particle.
- The BISON results quoted are the published ones; none is re-run here.
- The strain decomposition and the Weibull form are the generic textbook
  forms, written for orientation, not transcribed from BISON.

## References

- Kugeler, K., Nabielek, H. and Buckthorpe, D. (2017). *The High Temperature
  Gas-cooled Reactor: Safety considerations of the (V)HTR-Modul*. Eds.
  Scheuermann, W., Haneklaus, N. and Fütterer, M. EUR 28712 EN, Publications
  Office of the European Union, Luxembourg.
  [doi:10.2760/270321](https://doi.org/10.2760/270321). In the maintainer's
  open corpus (`theodore-open-corpus/jrc/kjna28712enn.pdf`); reuse authorised
  provided the source is acknowledged (Decision 2011/833/EU). Table 5,
  pp. 32–33, read 2026-10-21.
- Hales, J. D., Williamson, R. L., Novascone, S. R., Perez, D. M., Spencer,
  B. W. and Pastore, G. (2013). Multidimensional multiphysics simulation of
  TRISO particle fuel. *Journal of Nuclear Materials* 443(1–3).
  [doi:10.1016/j.jnucmat.2013.07.070](https://doi.org/10.1016/j.jnucmat.2013.07.070).
  Not in the corpus. Bibliographic data and abstract checked on the public
  OSTI record 1111031 on 2026-10-21; full text not read.
- Schappel, D., Pastore, G. and Terrani, K. A. (2021, online; issue dated 2022). Modeling interface
  debonding in coated fuel particles with BISON. *Nuclear Science and
  Engineering* 196(3), 276–284.
  [doi:10.1080/00295639.2021.1955590](https://doi.org/10.1080/00295639.2021.1955590).
  Not in the corpus. The DOE public-access accepted manuscript (OSTI) was read
  on 2026-10-21, sections 1, 2.1 and 3.2; section numbers refer to that
  manuscript.
- Jiang, W., Hales, J. D., Spencer, B. W., Collin, B. P., Slaughter, A. E.,
  Novascone, S. R., Toptan, A., Gamble, K. A. and Gardner, R. (2021). TRISO
  particle fuel performance and failure analysis with BISON. *Journal of
  Nuclear Materials* 548, 152795.
  [doi:10.1016/j.jnucmat.2021.152795](https://doi.org/10.1016/j.jnucmat.2021.152795).
  Cited in the maintainer's dissertation (Ong 2024, in the open corpus);
  bibliographic record checked on Crossref on 2026-10-21; full text not read.
- Ong, T. K. C. (2024). *Digital Twins as Testbeds for Iterative Simulated
  Neutronics Feedback Controller Development*. PhD dissertation, University of
  California, Berkeley. In the open corpus
  (`theodore-open-corpus/ong/ong2024-phd-dissertation-digital-twins-neutronics-feedback.pdf`).
  Pages 325 to 326 (heat capacities from Jiang et al. 2021) and its reference
  list read 2026-10-21. Public.
- Wells, B. E., Phillips, N. R. and Geelhood, K. J. (2021). *TRISO Fuel:
  Properties and Failure Modes*. PNNL-31427, Pacific Northwest National
  Laboratory, prepared for the U.S. Nuclear Regulatory Commission. In the
  maintainer's proprietary folder (`geelhood2021pnnl31427.pdf`). Pages 4.5,
  4.9 and 4.12 read 2026-10-21. Cited with a page and paraphrased, not copied.
- International Atomic Energy Agency (2012). *Advances in High Temperature Gas
  Cooled Reactor Fuel Technology*. IAEA-TECDOC-1674, Vienna. ISBN
  978-92-0-125310-1. In the maintainer's proprietary folder
  (`reports/iaea-tecdoc-1674-htgr-fuel-technology.pdf`). Printed pages 454,
  485, 489, 518 and 519 read 2026-10-21.
- U.S. Nuclear Regulatory Commission (2004). *TRISO-Coated Particle Fuel
  Phenomenon Identification and Ranking Tables (PIRTs) for Fission Product
  Transport Due to Manufacturing, Operations and Accidents*, Main Report.
  NUREG/CR-6844, Vol. 1. In the maintainer's proprietary folder
  (`2004nuregcr6844.pdf`). Pages 2-7, 2-31 and 2-32 read 2026-10-21.

## Next question

A finite-element model of every particle is what we would like, and it is not
here. **What is the simplest model that still captures failure?** Keep only
the part of the story that breaks particles in an accident: gas pushing on a
shell. [Step 6](./08-pressure-stress.md) simplifies the particle to a pressure
vessel.
