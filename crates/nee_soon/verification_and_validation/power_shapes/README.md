# Power shapes: Monte Carlo tally to thermal-hydraulic cells (gh:#1184)

Module: `nee_soon::power_shape`. Status: untrusted AI-generated draft, 2026-11-03. For research, education and V&V only.

## What is checked

The module turns a Monte Carlo `KappaFission` tally into a power shape, and puts a total power onto thermal-hydraulic cells. Three properties are checked:

1. The cells' powers add up to the total.
2. The expansions read back what was scored.
3. The frames convert correctly.

## Methodology

Tests in `src/power_shape_tests.rs` (`cargo test --release -p nee_soon --lib power_shape`):

- **Conservation:** all nine shape kinds (uniform, region lumps, chopped cosine, sine, `J0 x cos`, Legendre-z, Zernike-Legendre, spatial spherical harmonics, R-Z table) are put on three kinds of cell:
  - 48 rings of a TECDOC-frame grid;
  - 4 boxes;
  - 20 centroid points.
  - **Pass:** `sum q_c V_c` equals 10 MW to 1e-12 relative.
- **Exact ring integrals:** three checks against closed forms.
  - A 2 x 2 R-Z table on a ring that straddles every table edge. **Pass:** 1e-13.
  - The chopped cosine over `[-50, 70]` cm against `(200/pi)(sin 0.35 pi + sin 0.25 pi)`. **Pass:** 1e-12.
  - A region lump's fraction. **Pass:** 1e-14.
- **Expansion round trips:** an analytic power is scored through the Monte Carlo filter's own `expansion_moments` (12-point Gauss in `r` and `z`, 24 azimuths, on the cylinder `r < 90`, `|z| < 90` cm). `PowerShape::from_tally` then reads it back.
  - A cubic `lambda(z)` through `SpatialLegendreFilter`. **Pass:** coefficients to 1e-12.
  - An order-2 Zernike areal density through `ZernikeFilter`. **Pass:** coefficients to 1e-12. This checks the disc norms `pi/(n+1)` (`m = 0`) and `pi/(2(n+1))`.
  - The separable product of the two from two tallies. **Pass:** `q` itself at three points, 1e-11.
  - An R-Z mesh tally. **Pass:** densities to 1e-12.
- **Frames:** MC to TECDOC to MC, and MC to GeN-Foam metres and back, for half-heights 55, 61.788 and 90 cm.
  - The bed floor maps to `z_T = 180` cm at every loading.
  - The full core's bed top maps to `z_T = 0`.
- **On the `htr10_rz` mesh:** a `J0 x cos` on the 1124 bed cells conserves 10 MW to 1e-12.
  - The uniform shape's captured fraction of the cylinder is above 1, because the cone adds bed volume.
  - The sine's captured fraction, whose support is the cylinder, is 1 to 1e-10.

### Zernike to Fourier-Bessel (maintainer request, 2026-11-03)

`zernike_to_bessel` re-expresses the `m = 0` part of a Zernike shape in `J0(alpha_k r / R_b)` modes. No Monte Carlo Bessel filter is involved: the tally stays Zernike.

**How the coefficients are computed:**

- The projection uses the Bessel weight `r`.
- Dirichlet: Watson Sec 18.24, norm `J1(alpha)^2/2` (A&S 11.4.5).
- Neumann (Dini, `J1(alpha) = 0` plus the constant term): Watson Sec 18.33, norm `J0(alpha)^2/2`.
- Integration is composite 30-point Gauss to 1e-14.
- The zeros come from Newton on petir's `J0`/`J1`. The tests check them against A&S Tab 9.5 to 1e-13.

**Boundary condition:**

- **Neumann is the default.** A reflected core's Zernike reconstruction is non-zero at the edge, and only the Dini series carries an edge value.
- Dirichlet is an explicit choice. Its radius is `R_b >= R`.

**Tests:**

- **A pure `J0(2.405 r/R)`**, projected onto Zernike order 20 and converted with Dirichlet at `R_b = R`, gives one term:
  - `b_1 = 1.0000000000000004`;
  - `b_2 .. b_6` are at most `1.8e-12`.
- **An order-4 Zernike polynomial** (`c_00 = 1, c_20 = -0.3, c_40 = 0.1`): the L2 error is measured in the `r`-weighted norm over `[0, R_b]`, relative to the disc. The table gives the error and the disc-power error `dP` at each `n_terms`:

  | n_terms | Neumann L2 | Neumann dP | Dirichlet `R_b = R` L2 | dP | Dirichlet `R_b = 1.2 R` L2 | dP |
  |---|---|---|---|---|---|---|
  | 1 | 1.789e-1 | 2e-16 | 4.090e-1 | -22.7 % | 2.409e-1 | -8.9 % |
  | 2 | 1.631e-2 | 2e-16 | 3.254e-1 | -13.7 % | 2.101e-1 | -5.6 % |
  | 3 | 3.835e-3 | 2e-16 | 2.764e-1 | -9.7 % | 2.089e-1 | -5.5 % |
  | 4 | 1.363e-3 | 2e-16 | 2.438e-1 | -7.5 % | 2.026e-1 | -5.2 % |
  | 6 | 3.171e-4 | 2e-16 | 2.025e-1 | -5.2 % | 1.567e-1 | -3.1 % |
  | 10 | 5.093e-5 | 2e-16 | 1.587e-1 | -3.2 % | 1.279e-1 | -2.1 % |

  - The error falls monotonically in every column; the test asserts it.
  - **Neumann conserves the disc power exactly at every `n_terms`**, because only `b_0` integrates. The test asserts 1e-12.
  - Dirichlet converges slowly, as expected for a function that is non-zero where every mode vanishes.
- **Picture:** `images/zernike_vs_bessel_radial.png` shows the Zernike profile with the 1, 3 and 6-term reconstructions for both boundaries.
- **`PowerShape::BesselLegendre`** (a Bessel radial times a Legendre axial shape) conserves 10 MW on the rings to 1e-12.

## Results (2026-11-03)

- **Tests:** 11 passed, 0 failed (8 before the Bessel converter was added). Before the subdivision caps were lowered, the run took 119 s; the conservation test with spherical harmonics on boxes dominates.
- **`examples/power_shape_images.rs`:** every shape on the 1124 `htr10_rz` bed cells sums to `1.000000000000e7` W.

## Images

The images are in `images/`. There are three kinds:

- `*_rz.png`: R-Z (`x >= 0`, `y = 0`), MC frame, the full-core bed.
- `*_xy.png`: x-y at `z = 0`, for the non-axisymmetric shapes.
- `*_on_htr10_rz_mesh.png`: the 10 MW densities on the `htr10_rz` cells, cylinder and stepped cone, drawn from the mesh's own edges.

**What was checked by eye:**

- `j0_cosine_on_htr10_rz_mesh.png`: the peak is on the axis at mid-height, and the stepped cone below the bed is drawn.
- `zernike_legendre_xy.png`: the bowl.

**Not checked:** each picture's numbers against a hand calculation. The tests do that.

## Limitations

- **Point (centroid) cells:** the midpoint rule, not exact.
- **Boxes:** refined to 4 subdivisions per axis. A cylindrical discontinuity inside a box converges at first order.
- **Spatial spherical harmonics:** analytic only. No Monte Carlo spatial-SH filter exists; that is gh:#1189.
- **`ZernikeLegendre` from tallies:** assumes a separable power. Each expansion filter must be its tally's only filter, so the two expansions come from two tallies.
