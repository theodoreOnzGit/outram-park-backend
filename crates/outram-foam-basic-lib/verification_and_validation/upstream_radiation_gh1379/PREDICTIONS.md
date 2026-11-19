# Predictions, written and committed BEFORE the upstream driver or the Rust port was run (gh:#1379, 2026-11-13)

Cases: `hexA` (a = e = 2 /m, E = 0), `hexB` (a = 1.5, e = 1.2 /m, E = 500 W/m3) on the
316-cell skewed hex+prism blockMesh of gh:#1362 with inlet/outlet/walls Marshak patches
(outlet `MarshakRadiationFixedTemperature`, Trad = 900 K; emissivities 0.9 / 0.6 / 0.8);
`slab` (40 cells, 1 m, T = 1000 K, walls 1500 K and 500 K, eps = 0.8 / 0.5, a = 2 or 0.5 /m).
Upstream: ESI v2506 `radiationModel::New(T)` -> P1, PCG/DIC, tolerance 1e-10.

1. `physicoChemical::sigma` from the truncated etc/controlDict constants (k = 1.38065e-23,
   h = 6.62607e-34, c = 2.99792e8) is computed by the port in the same operation order and
   is bit-identical (prediction: 1/1). It differs from CODATA 5.670374e-8 by about 1e-5
   relative; the port uses the ESI value (as for the gas constant in gh:#1362).
2. Pure arithmetic quantities (Marshak `valueFraction`, `refValue`, `Rp`, `Ru` given G, the
   `ST` and `Sh` matrices given G) are bit-identical: 100 %.
3. `G`: the matrix is laplacian + Sp with the same boundary-coefficient forms already pinned
   bit-identical in gh:#1362, solved with the port's DIC-PCG run for exactly the iteration
   count ESI reports. Prediction: `G` bit-identical on all cells of hexA, hexB and the slab
   (100 %) PROVIDED the Sp term enters the diagonal before the boundary internalCoeffs, as in
   upstream; if the port adds it afterwards I expect a handful of 1-ulp differences in
   boundary cells only. `qr` then follows (100 %).
4. Stopping rule: the port's residual normalisation differs from ESI's `normFactor`, so I
   expect the port, given ESI's tolerance, to stop within +-3 iterations of ESI, not at the
   same count (gh:#1362 row 3 saw the same). Not gated.
5. Slab analytic (uniform-T medium, P1 + Marshak, closed form in the test): the discrete G
   converges to it at observed order between 1.8 and 2.1 when N doubles (10,20,40,80), and
   the N = 80 maximum relative error of G is below 1e-4. If the observed order is lower I
   will report that, not tune.
6. Isothermal enclosure (all walls and medium at the same T, e = a): G = 4 sigma T^4 to 1e-12
   relative, qr = 0 to 1e-9 of 4 sigma T^4 * gamma/delta.
7. Global balance: sum(V*(Ru - Rp*T^4)) + sum(qr*|Sf|) over boundary faces vanishes to
   1e-8 of sum(V*Rp*T^4) at PCG tolerance 1e-12 (it is the integral of the G equation).
