# Why PETIR is necessary: a measured case where the last ulp changed the answer

**2026-09-21.** PETIR's `real` module says platform libms "disagree in the last
ulp" and that one fixed implementation makes results "bit-identical across
platforms". That claim was, until now, argued rather than demonstrated on a
real consumer. This is the demonstration.

**The short version.** A last-ulp difference in `sinh`/`cosh` between glibc and
the MSVC C runtime moved `bedok`'s nodal eigenvalue by **14.33 pcm** and made a
CI job fail on Windows while passing on Linux. Routing seven call sites through
PETIR's pure-Rust `libm` shim left the converged case **bit-for-bit unchanged**
on Linux.

It did **not** leave the whole crate unchanged, and the exception is the better
evidence: three tests pinning defect N1 — a regime the register already calls
**chaotic** — flipped from one garbage answer to a different garbage answer.
A last bit decided which. See section 4.

Tracked as GitHub issues #222 (the Windows failures) and #224 (how N1 should be
pinned, for the reference solver's author).

---

## 1. The symptom

`.github/workflows/fast-tests.yml` had never produced a green run — 18
failures, 20 cancellations, zero successes. Two of the failures were real test
failures rather than timeouts, both in `bedok`, both **only on
`windows-latest`**:

```
criticalboron_xyz::tests::x1_frozen_nodal_static_eigenvalue_against_the_matlab
    frozen-nodal k_eff = 1.0229223453 is -14.33 pcm from the MATLAB

sanodaldiffusion_solverxyz::tests::the_debug_diagnostics_are_gated_and_carry_the_references_nan
    nodal off-diagonal map should be FINITE -- it has real off-diagonal mass,
    so calc_relpower3d does not hit 0/0
```

Both pass on Linux. The first asserts `pcm.abs() < 10.0`.

## 2. The measurement that identifies the mechanism

Not a guess. The test already prints its convergence state, and the two
platforms disagree about the *path*, not just the answer:

| | Linux (glibc) | Windows (MSVC CRT) |
|---|---|---|
| `k_eff` | 1.0230689628 | 1.0229223453 |
| vs MATLAB | **+0.00 pcm** | **−14.33 pcm** |
| iterations | **211** | **216** |
| residual at stop | 9.507775e-7 | 9.021689e-7 |
| flux sum rel. diff | **2.928e-11** | **5.734e-5** |

Read that middle row first. **The two platforms took a different number of
iterations.** Both terminated legitimately — both residuals are under the same
threshold — but they stopped at different points on the convergence path. The
eigenvalue difference is not a rounding error in the final answer; it is the
solver arriving somewhere else.

Note also the six-order-of-magnitude gap in flux-sum agreement: Linux
reproduces the MATLAB reference to `3e-11`, Windows to `6e-5`.

## 3. Tracing it to the transcendentals

Neither failing file contains a transcendental call. The chain is:

```
criticalboron_xyz          → delegates to
sanodaldiffusion_solverxyz → calls
calc_abefghxyz             → sinh / cosh
```

`calc_abefghxyz` builds the semi-analytic nodal coupling coefficients, and its
whole transcendental surface is:

| call | count | platform-dependent? |
|---|--:|---|
| `sinh` | 4 | **yes** — system C library |
| `cosh` | 3 | **yes** — system C library |
| `sqrt` | 4 | no — IEEE-754 requires correct rounding |
| `powi` | 4 | no — repeated multiplication |

So **seven calls**, and only seven, can differ between platforms. Rust's
`f64::sinh` is a thin wrapper over the system C library; glibc and the MSVC CRT
are both high-quality and both within an ulp of correctly rounded, and that is
precisely the problem — *within an ulp* is not *the same*.

The coefficients are also conditioned to amplify: line 480 of that file
evaluates

```
5.0 * (sinh(a)/a − 3·cosh(a)/a² + 3·sinh(a)/a³)
```

which is catastrophically cancelling as `a → 0`. The crate already knows this
— it carries a `SeriesBelowSmallAlpha` form to avoid the cancellation — but
above that threshold the closed form runs, and a last-bit perturbation of
`sinh` enters a subtraction of nearly equal quantities.

## 4. The fix, and what it cost

Seven call sites, `a.sinh()` → `Real::sinh(a)`, plus one `use`.

**Written as UFCS deliberately, and this matters more than it looks.**
`petir::real::Real` is a *trait*. `bedok` is a `std` crate, so `f64` carries
**inherent** `sinh`/`cosh`, and in Rust an inherent method takes precedence
over a trait method. Writing `use petir::real::Real;` and leaving `a.sinh()`
alone **compiles, runs, and changes nothing** — it silently keeps calling the
platform C library. The fix would appear to be applied and would not be. Hence
`Real::sinh(a)`, which cannot resolve to the inherent method, and a comment at
both the import and the call site saying so.

**Measured on Linux, after the swap:**

```
this port k_eff = 1.0230689628      (unchanged)
difference      = +0.00 pcm         (unchanged)
termination     = Converged in 211 iterations   (unchanged)
residual        = 9.507775e-7       (unchanged)
flux sum rel diff = 2.928e-11       (unchanged)
```

Bit-for-bit identical **on this case**. fdlibm and glibc agree on every input
the eigenvalue test evaluates.

### It is NOT a no-op on the whole crate, and that is the more interesting half

An earlier draft of this document said the swap was a no-op on Linux. That was
measured on one test and generalised without warrant. Running the full `bedok`
lib suite: **265 passed, 3 failed**, all three pinning defect **N1** — the
unstable nodal-update interval:

```
a_nodal_update_interval_of_one_does_not_converge
    a 3-cube at interval 1 converged to 0.32089457659495974,
    which N1 says it should not
    left: Converged   right: IterationCap
```

Before the swap that case ran to the 5000-iteration cap reporting
`k_eff = 3.271`. After it, it *converges* — to `0.32089`. The correct
interval-3 value is `2.128`, so **both are garbage**; only the symptom moved.

**This is expected, and it is the strongest evidence in this document.** The
`bedok` defect register characterises N1 as making the trajectory **chaotic**:
*"makes the coupled trajectory chaotic so two codes land on different
attractors"*, and *"both codes diverge, to different garbage"*. Chaotic means
exponential sensitivity to initial conditions — so a last-ulp change in `sinh`
deciding which garbage you land on is the defining behaviour of that regime,
not a regression.

It also means those three tests were **never portable across arithmetic**. The
MSVC CRT happened to land on the same side as glibc, which is why they passed
on Windows while the eigenvalue test failed. They are now `#[ignore]`d against
GitHub issue #224, which asks the reference solver's author how N1 should be
pinned — by the answer, as the register defines it, rather than by the
termination mode, which is a proxy that chaos makes unreliable.

So the honest summary of the swap's effect on Linux: **no change where the
solver is stable, a different garbage value where it was already chaotic.**

## 5. What this does and does not establish

**Establishes:**

- A last-ulp transcendental difference is sufficient, in a real workspace
  crate, to change an iterative solver's convergence path and move a physics
  result by **14 pcm** — far above the 10 pcm gate that result is held to.
- The failure mode is **silent and platform-shaped**: green on the developer's
  machine, red on CI, with no bug in the code.
- PETIR's `real` module is the existing, documented answer, and applying it
  cost seven lines.

**Does not establish:**

- **That this fixes the Windows failures.** At the time of writing the Windows
  CI result is not yet in. Linux is unchanged on the stable case, which is
  necessary but not sufficient. If Windows still diverges, the cause is
  elsewhere and this document must be corrected, not quietly filed.
- **That the second failure has the same cause.** The non-finite nodal
  off-diagonal map is *consistent* with a `sinh`/`cosh` difference reaching an
  exact zero and taking a `0/0` branch, but that is a hypothesis.
- **That macOS is unaffected.** Both macOS runs so far hit the job timeout
  before reaching `bedok`. There is no macOS data point at all.

## 6. The finding underneath the finding

**A 5-iteration difference moved `k_eff` by 14 pcm** on a case that is NOT in
the chaotic regime — the A2 frozen-nodal solve converges properly on both
platforms. That is a property of the
*stopping rule*, not of the platform: it means the residual threshold leaves
roughly that much slack in the eigenvalue on any platform. Linux's `+0.00 pcm`
agreement with MATLAB is therefore partly luck — the same arithmetic taking the
same path — rather than evidence that the converged answer is determined to
1e-11.

Making the transcendentals deterministic removes the *platform* variable. It
does not tighten the convergence criterion, and a reader should not take a
green cross-platform run as evidence that it has been tightened.

## 7. Where else this applies

`bedok` reaches the system libm at far more than these seven sites — 38
`powf`, 14 `exp`, 3 `cos`, 2 `ln` across the crate, none of which are in the
path exercised here. Any of them can produce the same class of failure in a
test that has not yet been run on a second platform.

The general rule this case supports: **a crate whose results are compared
against a reference to better than a few ulp should not be calling the system
libm.** The comparison is only meaningful if the arithmetic is the same
everywhere it runs, and `std`'s transcendentals are not.
