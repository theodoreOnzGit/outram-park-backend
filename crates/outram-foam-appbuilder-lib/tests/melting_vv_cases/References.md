# References — melting / solidification V&V cases

Provenance record for `tests/melting_vv_cases/`, per the workspace `CLAUDE.md`
data-provenance rule and `DATA_POLICY.md`.

---

## Summary of data status

| Case | Reference | Status |
|---|---|---|
| 1-D Stefan problem | Closed-form similarity solution | **Available** — derived, not retrieved |
| Energy conservation | Discrete balance of the scheme itself | **Available** — no external data needed |
| Gallium melting cavity | Gau & Viskanta (1986) | **GAP — no data retrieved, none used** |

---

## 1. Stefan similarity solution (Cases 1 and 2)

- **Nature of the reference.** Mathematics, not measured data. The one-phase
  Stefan problem has the closed-form solution

  $$ s(t) = 2\lambda\sqrt{\alpha_{th} t}, \quad \lambda e^{\lambda^{2}}\mathrm{erf}(\lambda) = \frac{St}{\sqrt{\pi}} $$

  where `St = Cp(T_w - T_m)/L`. This is a standard result reproduced in every
  heat-transfer textbook and derivable from the same conservation laws the code
  discretises.
- **Why this matters here.** Because it is derived rather than retrieved, it is
  usable with network egress blocked, and it carries **no licence or access
  restriction** — no third-party data enters the repository.
- **Material properties.** Deliberately synthetic round numbers
  (`Cp = 1000 J/(kg·K)`, `L = 1e5 J/kg`, `α_th = 1e-5 m²/s`, `ΔT = 20 K`,
  giving `St = 0.2` exactly). They are **not** claimed to be any real
  substance. Permitted by `DATA_POLICY.md` as "synthetic data generated for
  educational or verification purposes".
- **Processing / digitisation steps.** None. `λ` is computed inside the test by
  bisection on the transcendental equation above; no value is transcribed from
  any source.
- **`erf` implementation.** Abramowitz & Stegun, *Handbook of Mathematical
  Functions*, formula 7.1.26 (US National Bureau of Standards, 1964). A US
  Government work, in the public domain. Stated accuracy |ε| < 1.5e-7, which is
  far below the ~1e-3 tolerances of the comparison.
- **Assumptions and limitations.** The analytical solution assumes a sharp front
  and a semi-infinite domain. The numerical model uses a finite mushy interval
  and a finite domain; the domain is sized so the front never approaches the far
  wall, and the mushy interval is swept as a convergence parameter. The residual
  ~0.025 % disagreement is attributed to the mushy interval, which is a
  deliberate model idealisation.

---

## 2. Gallium melting cavity — **GAP, no reference data**

### What is missing, and why

The intended benchmark is:

> Gau, C. and Viskanta, R. (1986), "Melting and Solidification of a Pure Metal
> on a Vertical Wall", *Journal of Heat Transfer* **108**(1), 174–181.
> DOI: 10.1115/1.3246884

**This paper was not retrieved and none of its content is used anywhere in this
repository.** Network egress is blocked in the build container. Verified
2026-08-05 by direct test — `doi.org`, `asmedigitalcollection.asme.org`,
`scholar.google.com` and general reference hosts all returned HTTP 000 with the
agent proxy reporting `connect_rejected — gateway answered 403 to CONNECT
(policy denial)`.

The vendored OpenFOAM tree
(`crates/outram-foam-turbulence-lib/upstream_source/OpenFOAM`) was searched as a
fallback source of case parameters: `tutorials/` contains **no** case matching
`solidification`, `melting` or `gallium`, so there was no upstream tutorial to
take inputs from either.

### Consequences, stated plainly

1. **No melt-front position, Nusselt number, or any other measured quantity from
   Gau & Viskanta appears in the test, the code, or the documentation.**
   Fabricating one would be the single worst thing that could be done here, so
   none was invented.
2. **The material properties in `GalliumCase::default` are UNVERIFIED.** They
   are recalled values of roughly the right order for pure gallium and could not
   be checked against any citable source. They are placeholders, not data, and
   are collected in one struct so they can be replaced wholesale without
   touching solver or test logic.
3. The gallium test therefore asserts only **configuration-independent physics**
   — melting proceeds, melt leads at the hot wall, convection develops, the
   front tilts deeper at the top than the bottom, temperatures stay inside the
   wall range — and *reports* its quantitative output without judging it.
4. The case is labelled a **demonstration**, not a verification case, and
   certainly not a validation.

### Geometry and boundary conditions actually used

Chosen to match the *configuration* described in the general literature (a
rectangular cavity with one vertical wall above and the opposite wall below the
melting point), **not** transcribed from the paper:

| Item | Value | Source |
|---|---|---|
| Cavity width x height | 0.0889 x 0.0635 m | UNVERIFIED placeholder |
| Mesh | 40 x 30 cells, 2-D (`empty` front/back) | This project's choice |
| Hot wall (left) | 311.0 K, no-slip | UNVERIFIED placeholder |
| Cold wall (right) | 301.3 K, no-slip | UNVERIFIED placeholder |
| Top / bottom | adiabatic, no-slip | Configuration convention |
| Melting point | 302.8 K | UNVERIFIED placeholder |
| Mushy interval | 0.2 K | Numerical necessity, not physical |
| Run duration | 60 s at dt = 0.005 s | Sized to fit a test suite |

The mushy interval deserves separate mention: pure gallium has **none**. A
finite interval is an artefact the enthalpy-porosity method requires, so it is a
modelling assumption introduced by this project, not a property of the material.

### What would close this GAP

1. Retrieve Gau & Viskanta (1986) through an institutional subscription or an
   open-access mirror.
2. Record, in this file: the cavity dimensions, wall temperatures and the exact
   property set the paper used; the licence/access terms of the source; the URL
   or DOI; the date accessed.
3. Digitise the published melt-front positions at the published times, recording
   the digitisation method (plot-digitiser software, axis calibration points,
   estimated reading uncertainty) and any assumptions.
4. Replace `GalliumCase::default` with the sourced properties.
5. Convert the five qualitative criteria into a quantitative front-position
   comparison with a stated tolerance and an uncertainty estimate.
6. Only then may the case be described as a verification case against a
   benchmark — and validation remains the human maintainer's decision, per
   `VERIFICATION_AND_VALIDATION.md`.

---

## Compliance notes

- No confidential, restricted, proprietary, operational or unpublished data is
  used in any case here (`DATA_POLICY.md`).
- All quantitative results recorded in the test doc comments were produced by
  actually running the tests in release mode on 2026-08-05; none is predicted,
  estimated or carried over from another source (`RESPONSIBLE_USE.md`,
  "Don't fabricate or overclaim").
- All cases here are **verification**. Validation is not claimed for any of them.
