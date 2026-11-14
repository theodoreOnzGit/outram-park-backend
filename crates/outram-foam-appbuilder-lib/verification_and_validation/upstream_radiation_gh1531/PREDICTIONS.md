# Predictions, written 2026-11-13 BEFORE any ESI run of the radiation case and BEFORE any test run (gh:#1531)

Case (fixed before running): the gh:#1363 two-region case (water fluid 20x5x1, steel-like solid 10x5x1,
dt 0.1 s, 50 steps, fluid at rest, frozenFlow) with P1 radiation switched on in the FLUID only:
a = e = 50 /m, E = 0, G initial uniform 0, wall emissivities left 0.9 / interface 0.8 / walls 0.5,
`laplacian(gammaRad,G)` Gauss linear uncorrected, G solver PCG/DIC 1e-15 (the port replays ESI's per-step
iteration count with tolerance 0). Interface `qr none` on both sides (the port has no qr term in its
coupled BC; stated as not covered).

Derivations (from the equations, before data):

1. Step 1, G starts at 0 so Ru = 0: the medium emits and does not absorb. Cooling of a cell away from
   the walls, ESI = port: dT = -Rp T^4 dt/(rho cp) / (1 + 4 Rp T^3 dt/(rho cp)), Rp = 4 e sigma,
   sigma = 5.670408558e-8: Rp T^4 = 200*5.670408558e-8*8.1e9 = 9.186e4 W/m3, dT = -2.197e-3 K,
   implicit factor 1 - 2.9e-5. Predicted fluid mean dT at step 1 = -2.197e-3 K (+-2 %; cells next to the
   interface/left wall differ by wall exchange, so the MEAN over the 100 cells is quoted).
2. Radiation effect is detectable: port-without-radiation vs ESI-with-radiation fluid cell T differs by
   >= 1e-3 K at some step (step 1 alone gives 2.2e-3 K) and <= 0.12 K at step 50 (upper bound: the medium
   seeing 360 K everywhere, a*4 sigma (360^4-300^4) dt_total/(rho cp) = 0.12 K). If outside, the case cannot discriminate.
3. Port with radiation vs ESI, fluid cell T: <= 5e-11 K at every step (the no-radiation floor was 1.05e-11 K, from
   T-form vs h-form representation; radiation adds a source of size 2e-3 K per step whose own round-off is
   ~1e-16 K, so no new floor expected). Solid T: <= 2e-11 K.
4. G (cells): relative worst <= 1e-9 at every step with ESI's iteration counts replayed (G inherits the T difference
   through 4 e sigma T^4: dG/G ~ 4 dT/T ~ 1e-13). qr on the left wall: <= 1e-8 of max|qr| (qr = difference of
   close numbers near equilibrium, so a looser bound).
5. WHERE IT FIRST DIVERGES (expected, not yet measured): the Marshak refValue 4 sigma Tw^4 on the INTERFACE
   patch. The gh:#1363 record found ESI's stored T.boundaryField() at the coupled interface lags the BC coefficients
   by one step (the port's `evaluate_boundary_values` yields the current-step value). Radiation reads the stored
   boundary value, so I predict the first divergence at step 2, in G of the cells next to the interface, of
   size 4 sigma 4 T^3 dTf ~ up to 10 W/m2 with dTf = the one-step change of the interface face temperature, unless the
   port is made to use the pre-solve boundary value. Left wall (fixedValue) and `walls` (zeroGradient) cannot diverge this way.
   If instead step 1 diverges, the suspect order is: initial-G convention, Tref/he conversion, then warm start.
6. Defaults: radiation off, every existing pin (gh:#1363 1.05e-11, gh:#1156 energy balance) unchanged bit for bit
   (the new field is None; no arithmetic is added to the off path).
7. `solverFreq 3` ablation: G recomputed only on steps 3,6,... (and the first call): the port reproduces ESI's
   `timeIndex % solverFreq` rule (unit test, ESI not re-run for it).

---

# Addendum, written 2026-11-14 BEFORE any ESI run with `qr`/`qrNbr` and BEFORE any test of the new code (gh:#1531, qr coupling)

Case: the same case and parameters as above (a = e = 50 /m, 50 steps, dt 0.1 s), with the interface condition
switched to ESI's tutorial convention (`solarBeamWithTrees`): the FLUID interface patch `qr qr; qrNbr none`, the
SOLID interface patch `qr none; qrNbr qr`. ESI's `turbulentTemperatureRadCoupledMixed::updateCoeffs` (read,
v2506) then sets `refGrad = (qr + qrNbr)/kappaTp` on each side and leaves `valueFraction = KDeltaNbr/(KDeltaNbr+alpha)`,
`refValue = TcNbr` unchanged. `qr` is the radiation model's volScalarField `qr`, boundary values
`-gamma snGrad(G)` (P1.C:244). No `qrRelaxation` exists in this BC in v2506 (it is only in `thermalBaffle1D`).

1. Q1 (default-on wiring): `refGrad` on the fluid side is `qr_fluid[face]/kappa_f` and on the solid side the paired
   face's `qr_fluid/kappa_s`; `valueFraction` and `refValue` are bit-unchanged from the qr-less port.
2. Q2 (bit identity): port with qr vs ESI with qr, worst fluid T <= 5e-11 K and solid T <= 2e-11 K at every step, G rel
   <= 1e-9, qr rel <= 1e-8 of max|qr| (same floors as the qr-less run, 1.07e-11 K / 3.6e-12 K / 1.5e-13 / 2.8e-10).
   Risk named in advance: if upstream evaluates the BC again after `rad.correct()` the lag differs and a step-1 or
   step-2 divergence of order |dqr|/kappa appears (I expect NOT: in EEqn.H `thermo.correct()` precedes `rad.correct()`).
   If so, the order of evaluation, not the formula, is the suspect.
3. Q3 (the term matters): ESI-with-qr vs ESI-without-qr (the committed gh:#1531 reference) fluid cell T differs by
   >= 1e-3 K at the interface-adjacent cells at step 50 (the interface wall is hotter than the medium, so G at the wall
   exceeds G in the cell, `snGrad G > 0`, `qr < 0`). Sign: the interface-adjacent fluid cell is LOWER with qr than
   without. Size: an interface radiative flux bounded by epsilon sigma (360^4 - 300^4) ~ 3.8e2 W/m2 acting for 5 s on a
   water cell row 0.1 m deep gives <= 4.5e-3 K, so I expect 1e-3 .. 1e-2 K, not more.
4. Q4 (ablation discriminates): the port with the qr coupling ablated (`without_radiative_flux`) differs from ESI-with-qr
   by >= 1e-3 K (it must, if Q3 holds), and the port WITH the ablation equals the committed qr-less ESI reference to the
   same 1.07e-11 K floor (so the old record is reproduced by an explicit ablation, not by the default).
5. Q5 (default): constructing through `ChtMultiRegionFoam::new` with a radiating fluid applies qr on both sides with no
   extra call; with a fluid without radiation nothing changes bit for bit (every gh:#1363 pin).
6. Q6 (not covered, stated before measuring): `thicknessLayers`/`kappaLayers` with qr. The port's layered mode follows
   `turbulentTemperatureCoupledBaffleMixed`; `RadCoupledMixed` combines the layers differently
   (`alpha = kappa (1 + KDeltaNbr/KDeltaC) deltaCoeffs`), so I predict they do NOT agree and do not test them.
