# Side path: the Lagrangian random walk (coming)

> **Research, education and V&V only** (`RESPONSIBLE_USE.md`).

> **Review status:** AI-assisted first draft, 2026-10-04, not yet reviewed by
> a human. Built from commit
> [`@@COMMIT_SHORT@@`](https://github.com/theodoreOnzGit/outram-park-backend/commit/@@COMMIT@@).

> **Coming.** FLEXPART work is **deferred** by the maintainer (gh:#410,
> 2026-09-29). This page says what exists and what does not, so that nobody
> mistakes a verified set of kernels for a working dispersion model. It will
> become a full lesson when #410 is picked up.

## The question

Rungs 1–4 answer "where does the cloud go?" with a formula: a Gaussian whose
width comes from a fitted curve. The other answer is to **simulate it**:
release many computational particles, move each one with the mean wind plus a
**random turbulent kick**, and count where they end up. That is a Monte Carlo
random walk, the same idea the [Monte Carlo deep
dive](../../monte-carlo/index.html) uses for neutrons: free flights and random
collisions there, mean wind and random velocity fluctuations here. Its
statistics come from a pseudo-random generator in both cases (`petir`'s LCG).

```text
x(t + dt) = x(t) + (u_mean + u') dt
u'(t + dt) = r u'(t) + sqrt(1 - r^2) sigma_u xi,      r = exp(-dt / T_L),  xi ~ N(0, 1)
```

In its simplest form (homogeneous turbulence; FLEXPART's vertical component adds a drift correction for turbulence that varies with height), the turbulent velocity `u'` is a **Langevin** process: it remembers its last
value over a Lagrangian time scale `T_L` and is refreshed by a normal deviate
`xi`. The particle cloud's spread is not fitted; it *emerges* from the
turbulence statistics (`sigma_u`, `T_L`), which FLEXPART takes from
boundary-layer similarity theory and gridded meteorology.

## What the workspace has

`changi::flexpart` is a port of FLEXPART v10.4 (GPL-3.0-or-later, commit
`3d7eebf`), 34 modules, verified **kernel by kernel** against the compiled
Fortran built twice (as shipped in single precision, and with
`-fdefault-real-8`), [`docs/flexpart-code-to-code.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/docs/flexpart-code-to-code.md):

- the first kernels (2026-09-15, 1 812 cases, 14 groups) bit-exact against
  the double-precision build;
- stages 2–7 (2026-10-02), including the particle step `advance`, bit-exact
  except 5 of 7 320 outputs within 3.0e-15 (`petir`'s `erf` in place of
  FLEXPART's);
- the **stochastic** comparison: 20 000 particles in five turbulence regimes,
  16 replicates; criterion fixed before running, the port's mean and variance
  of displacement within 1 sigma of FLEXPART's run-to-run spread: **30 of 30
  within 1 sigma, chi-squared 19.7 on 30 degrees of freedom**. Getting there
  changed the port: FLEXPART clips every normal deviate to `[-3, 3]`
  (`gasdev1`), which narrows every turbulent velocity distribution by 0.5 %,
  and the port now applies the same clip
  ([`limit_rannumb`](../../../api/changi/flexpart/advance/fn.limit_rannumb.html)).

## What it does not have

- **No meteorology input.** The GRIB/NetCDF readers and the file writers are
  not ported; the kernels take decoded fields as arguments. The module cannot
  be pointed at real weather.
- **No driver.** Nothing in the dispersion chain, the examples or
  `htgr_sim_v1` runs FLEXPART. The chain's transport is the Gaussian puff
  (rung 4).
- **No validation**, as for everything else on this track.

So it is "a verified set of FLEXPART's kernels", not "FLEXPART in Rust".

**Back to the ladder:** [Rung 4](./04-puffs.md) ·
[Rung 5](./05-deposition.md).
