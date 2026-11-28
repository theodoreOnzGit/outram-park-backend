# boon-lay particle mechanics: verification record

> **Research, education and V&V only.** Not for reactor operation, licensing,
> fuel qualification, safety decisions or emergency response.
>
> **Review status:** AI-written first draft, 2026-11-28, not reviewed by a
> human. This is **verification only**. Nothing here is validated against a
> measurement, a benchmark or BISON's output.

Code: `crates/boon-lay/src/triso_particle_mechanics/`. Tests:
`src/triso_particle_mechanics/tests.rs` (18 tests) and farrer-park's
`quadrature::tests::line_rule_exactness_order_holds`.
To reproduce the numbers below:

```bash
cargo test --release -p boon-lay --lib triso_particle_mechanics -- --nocapture --test-threads=1
```

## What is being verified

boon-lay particle mechanics is a spherically symmetric TRISO particle model,
written from scratch from the equations of the **BISON Theory Manual** (Hales
et al. 2013, INL/EXT-13-29930; in the maintainer's private corpus). It is
**not BISON**: BISON's source was never obtained or read. It runs on the
finite-element layer of `farrer-park` (CSR matrix, sparsity pattern, Gauss
rule, ILU(0) factorisation).

The model solves equilibrium in each layer (manual Eq (2.6), spherical
strains Eqs (5.2)–(5.4), linear part) with the strain split into elastic,
thermal, irradiation dimensional change (Eqs (10.1)–(10.3)) and irradiation
creep (Eqs (10.4)–(10.7)), integrated by backward Euler. It also solves
steady conduction (Eqs (2.1)–(2.2) with no time derivative), and integrates a
Weibull weakest-link failure probability over the volume of each layer. The
Weibull form is not in the manual.

## Methodology

**Every input is synthetic.** The radii (IPyC 200–240 um, SiC 240–275 um,
OPyC 275–315 um; a five-layer conduction stack out to 465 um) and the
elastic constants (PyC 30 GPa, $\nu = 0.23$; SiC 400 GPa, $\nu = 0.13$) are
round numbers, not cited and not material properties. A verification test
compares the finite-element answer with a closed form **on the same inputs**,
so the inputs need no provenance. **None of these numbers may be quoted as a
property of PyC or SiC.** The manual gives no elastic constants for them.

Each reference is derived independently of the code it checks:

| Check | Reference | Pass criterion |
|---|---|---|
| Lamé, one shell, $p_{in} = 10$ MPa, $p_{out} = 0.1$ MPa | The classical thick-sphere formulas, written out in the test, not through `analytic.rs` | Displacement order > 1.9; stress order > 0.9 at all Gauss points and > 1.9 at element midpoints (n = 32 → 64) |
| Three bonded layers, thermal mismatch (300 → 1300 K) plus 20 MPa | `analytic::LayeredLameSolution`, $u = A r + B/r^2$ per layer, interface continuity | As above |
| Anisotropic irradiation strain (BISON 2013 dense PyC at $2 \times 10^{25}$ n/m²) | The closed form with the $C r \ln r$ term, $C = 4\mu(a_r - a_\theta)/(3(\lambda + 2\mu))$, derived in `analytic.rs` | As above |
| Solid sphere under outer pressure | $\sigma_r = \sigma_\theta = -p$, $u = -p(1 - 2\nu)r/E$ | Exact to 1e-8 (linear in $r$) |
| Free layer with an isotropic eigenstrain | Stress-free, $u = \varepsilon r$ | 1e-8 |
| Volume-conserving creep under constant pressure | Stress stays at Lamé; $u_{cr} = 3K\Phi(p_{in} - p_{out})a^3 b^3 / (4(b^3 - a^3) r^2)$ | Stress drift and creep displacement error < 1e-4; 1 step vs 20 steps < 1e-6 |
| Printed vs volume-conserving creep sign, hydrostatic stress | Volume creep $0$ and $-6K\Phi p$ | 1e-12 and 1e-6 |
| Eqs (10.5)–(10.6) transcription | Hand arithmetic at 1000 °C | 1e-12 |
| Closed-form strains vs the printed rates | Composite Simpson of Eqs (10.1)–(10.3) to $5 \times 10^{25}$ n/m² | 1e-10 |
| Steady conduction, 5 layers, kernel heat source | The layered closed form derived in `thermal.rs` | Order > 1.75 and error < 2e-4 of the temperature rise at n = 64 per layer |
| Weibull integral, one shell, 20 MPa | Simpson on 20 000 intervals of the Lamé $\sigma_\theta$ | Order > 1.8, error < 1e-4 at n = 64, $P_f$ not saturated |
| Time convergence, coupled IPyC/SiC/OPyC with BISON 2013 PyC laws, SiC creep, pressure ramp to 30 MPa | Self-convergence of the SiC's end-of-life peak $\sigma_\theta$, 10 to 160 steps | Observed order within 0.1 of 1 (backward Euler) |
| Mechanism: PyC shrinkage | Physics sign, predicted before running | SiC $\sigma_\theta < 0$ |

## Results (2026-11-28, mean-dilatation formulation)

| Check | Measured |
|---|---|
| Lamé, one shell, n = 64 | displacement 8.19e-7 (order 2.000); stress at all Gauss points 5.20e-3 of $p_{in}$ (order 0.996); midpoints 3.81e-6 (order 1.991) |
| Thermal mismatch, n = 64 | displacement 1.82e-8; stress 6.54e-3 of $p$ (all points); midpoints 4.89e-6 |
| Anisotropic PyC strain, n = 64 | displacement 1.74e-6; stress 9.70e-3 (all) and 1.34e-5 (midpoints) of $E_{SiC} \times 10^{-3}$ |
| Solid sphere | $u(b) = -2.500000 \times 10^{-9}$ m, matching the closed form |
| Volume-conserving creep, 128 elements | stress drift 1.25e-7 of $p$; creep displacement error 3.8e-7; 1 step vs 20 steps differ by 1.0e-10 |
| Creep sign, hydrostatic 10 MPa, $10^{25}$ n/m², 1000 °C | volume-conserving: 0; as printed: $-1.482 \times 10^{-2}$, which equals $-6K\Phi p$ |
| Eq (10.5)–(10.6) | $K = 2.4708 \times 10^{-35}$ (Pa·n/m²)⁻¹ at 1000 °C and 1.9 g/cm³ |
| Strain at $5 \times 10^{25}$ n/m² | buffer $-0.1006$ (both directions); dense PyC radial $+0.0785$, tangential $-0.0671$ |
| Conduction, 64 per layer | 4.39e-3 K on a 48.18 K rise (order 1.79, rising: 1.64, 1.71, 1.76, 1.79) |
| Weibull, n = 64 | $P_f = 1.103725 \times 10^{-3}$ vs $1.103730 \times 10^{-3}$, relative error 5.1e-6 |
| Time convergence | peak SiC $\sigma_\theta \to -8.70 \times 10^{7}$ Pa; observed orders 1.24, 1.15, 1.08 |
| PyC shrinkage, $0.5 \times 10^{25}$ n/m² | SiC $\sigma_\theta$ from −508 to −444 MPa (synthetic constants: the sign is the result, not the size) |

**Interpretation.** The discretisation converges at the rates two-node
elements should give: displacement at second order, stress at first order
pointwise and at second order at element midpoints, and creep at first
order in fluence. The closed forms are matched to the expected
discretisation error. That verifies that the equations as implemented are
solved correctly. It does **not** show that the BISON 2013 laws, or this
model, represent a real particle.

## What the verification found, and what changed

Each item below is a fault or a wrong prediction found by the first runs. Each
was fixed in the code or the test. No pass criterion was relaxed to get a pass.

1. **Volumetric locking under creep.** With full integration, the 128-element
   creep case moved the outer Gauss points' stress by **32 % of $p$**, while
   the midpoints moved 2.3e-4 and the displacement stayed within 4.7e-5. PyC
   creep has $\nu_c = 0.5$, so a creeping layer tends to incompressibility.
   **Fix:** the mean-dilatation (B-bar) strain, now the default
   (`Formulation::MeanDilatation`, pinned by `mean_dilatation_is_the_default`);
   full integration is kept as the ablation
   (`full_integration_locks_under_incompressible_creep`). The change moved
   the time-convergence case's peak SiC stress from $-8.31 \times 10^{7}$
   to $-8.70 \times 10^{7}$ Pa, which is 4.7 %.
2. **A residual tolerance on a direct solve.** On a tridiagonal matrix
   ILU(0) is the exact LU factorisation, and its residual is roughly machine
   epsilon times the condition number: measured at 2.1e-12 (64 elements,
   elastic) and 2.6e-10 (128, after creep). Those exceed farrer-park's CG
   gate of 1e-12, so the gate reported non-convergence for an exactly
   factorised system. **Fix:** the factorisation is applied as a direct
   solve with one refinement step (`mechanics::direct_solve`); accuracy is
   judged by the closed forms.
3. **Wrong predictions.** The first draft predicted second-order stress at
   every Gauss point (it is first order: 0.97–0.99 measured), nodally exact
   conduction (false in spherical coordinates: order 1.64–1.79), and a
   hand value of $K_0$ at 1000 °C, $1.23944 \times 10^{-29}$ (the sum is
   $1.2354 \times 10^{-29}$; the code was right). The Weibull case first ran
   at 100 MPa, where $P_f$ saturates at 1 and the check meant nothing; it now
   runs at 20 MPa and asserts $P_f$ is not saturated.

## Findings in the manual, for the maintainer (gh:#1965)

- **Eq (10.4)'s sign.** The manual prints $\dot{\varepsilon}_1 = K[\sigma_1 +
  \nu_c(\sigma_2 + \sigma_3)]\dot{\Phi}$ with $\nu_c = 0.5$. With a $+$, a
  hydrostatic stress creeps in volume ($-1.48 \times 10^{-2}$ here). A creep
  Poisson's ratio of 0.5 is meant to conserve volume, so the default is the
  $-$ form, and the printed form is `CreepLawSign::AsPrinted`. References
  [55] and [56] of the manual were not read.
- **The unit of $K$ in Eqs (10.5)–(10.6) is not printed.** It is read as
  (MPa·n/m²)⁻¹. That is an inference from magnitude: the Pa reading would give
  creep strains near $10^4$ for 100 MPa over $10^{25}$ n/m².
- **The SiC $K$ of Eq (10.7):** the manual notes that the cited figure
  suggests values about one tenth of the text values. The text values are
  the constants; Eq (10.7) is scalar and gives no creep Poisson's ratio.

## V&V data in the BISON manual itself

`bison-manual-vv-data.md` (this folder) lists every figure and table in the
manual and what each can test. The manual has **no** figure or data for its
TRISO laws, so they are checked against hand evaluations of the printed
formulas (`bison_manual_section_10_hand_values`). One figure, Fig. 8.7
(MATPRO UO2 swelling, relevant to a UO2 kernel), was digitised. It checks the
existing `outram-park-fork-offbeat` correlation
(`tests/bison_manual_fig87_matpro_swelling.rs`). The shape agrees within 0.003
at five temperatures.

## Geometry drawn (boon-lay AGENTS.md rule)

`particle_mesh.png` (from `examples/triso_particle_mechanics.rs`) draws
the two assembled meshes the tests use. Each annulus is coloured from
`ParticleMesh::element_layer`, so the picture shows what the solver
integrates over. Every element boundary and every node is drawn, and each
layer's radii and element count are in the legend.
`pyc_shrinkage_stress.png` plots $\sigma_r$ and $\sigma_\theta$ at every Gauss
point for the PyC-shrinkage case.

**Checked by looking (2026-11-28):**

- The layers are contiguous and in the right order.
- Element counts and radii match the legend.
- $\sigma_r$ is continuous across each interface and zero on both free
  surfaces. $\sigma_\theta$ jumps at the interfaces.
- The SiC is in compression and both PyC layers are in tangential tension.

**Not checked:** any real particle geometry. None was built, because the
dimensions here are synthetic.

## Not done

- **No validation.** No comparison with BISON output, the IAEA CRP-6
  benchmark cases or AGR data. The case data (IAEA-TECDOC-1674) are in the
  private corpus but were not read in this session.
- **Not implemented:** cited PyC/SiC elastic constants, thermal expansion and
  conductivities. The manual has none. ~~and no cited values are in the
  workspace~~ **CORRECTED 2026-11-28:** `outram-park-fork-offbeat` already
  ports OFFBEAT's PARFUME buffer, PyC and SiC correlations. They are not wired
  in, because that needs a new `boon-lay -> offbeat` dependency; gap heat transfer and contact or
  debonding (Sections 13.1–13.2); finite strain; transient conduction; kernel
  swelling and CO production for particle fuel; asphericity.

## References

- Hales, J.D., Novascone, S.R., Pastore, G., Perez, D.M., Spencer, B.W.,
  Williamson, R.L. (2013). *BISON Theory Manual: The Equations Behind Nuclear
  Fuel Analysis*. INL/EXT-13-29930, Idaho National Laboratory. Maintainer's
  private corpus (`propreitrary-literature-theodore`,
  `reports/inl-ext-13-29930-bison-theory-manual-2013.pdf`). Sections 1, 2, 5,
  6, 10.1, 10.2 and 13.3 read on 2026-11-28. No reuse licence; equations cited
  by number, no prose copied.
- Weibull, W. (1939). A statistical theory of the strength of materials.
  *Ingeniörsvetenskapsakademiens Handlingar* 151. Not in the corpus and not
  read; cited only for the standard weakest-link form.
