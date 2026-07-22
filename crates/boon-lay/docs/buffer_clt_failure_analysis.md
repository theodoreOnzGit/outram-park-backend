<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

# Why the single-Gaussian (central-limit) diffusion step fails in the buffer layer

**Status:** diagnosis note motivating the Walk-on-Spheres / first-passage
rewrite. This is an *analysis*, not a benchmark record — benchmark results live
under `verification_and_validation/`.

## Summary

The current Lagrangian diffusion step lumps many microscopic collisions into a
single Gaussian displacement whose per-component standard deviation is
$\sigma \approx \sqrt{2 D \, \Delta t}$. That Gaussian is only the *free-space*
diffusion propagator: it is accurate when the step is small compared with the
distance to the nearest material interface. In the TRISO **buffer** layer the
diffusion coefficient is large and the layer is thin, so with the second-scale
timesteps the simulator uses, $\sigma$ is comparable to or larger than the
buffer thickness. A single step then **teleports the atom clear across the
buffer** (and sometimes across the whole particle), never sampling the interface
it should have struck — and never feeling the SiC reflection that is the physical
reason SiC is the containment layer. Shrinking $\Delta t$ restores accuracy but
costs $O(1/\Delta t)$ steps, which is why the buffer dominates run time.

## The two propagators

For a walker at position $x$ in a homogeneous medium with diffusion coefficient
$D$, the exact probability density of its displacement after a time $\Delta t$,
*ignoring boundaries*, is the free-space Gaussian

$$p(\Delta x) = (4 \pi D \, \Delta t)^{-3/2} \exp\left(-\frac{|\Delta x|^2}{4 D \, \Delta t}\right),$$

with per-component variance $\sigma^2 = 2 D \, \Delta t$. Summing many small
isotropic collision steps converges to this Gaussian by the central limit
theorem — that is the theorem the current code invokes, and it is correct *as a
statement about an unbounded medium*.

The physical domain is **not** unbounded. Each TRISO layer is a spherical shell
with a different $D$, and the concentration and flux must stay continuous across
every shell interface. The correct propagator is the *Green's function of the
diffusion equation for the bounded, piecewise-homogeneous domain*, not the
free-space Gaussian. The Gaussian is its short-time, boundary-far limit — valid
only while

$$\sigma = \sqrt{2 D \, \Delta t} \ \ll\ \ell,$$

where $\ell$ is the distance from $x$ to the nearest interface. When that
inequality is violated the Gaussian moves probability mass to places the real
walker could only reach by crossing an interface first.

## The buffer numbers (CRP-6 geometry)

Nominal CRP-6 TRISO geometry, as constructed by `TrisoCell::new_crp6_geometry`:

| Layer | Thickness | Outer radius | Representative $D$ (m$^2$/s) |
|---|---|---|---|
| Fuel kernel | (radius) 212.5 µm | 212.5 µm | $\sim 5.6\times10^{-8}$ (Cs) |
| Buffer | 100 µm | 312.5 µm | $\sim 1\times10^{-8}$ |
| IPyC | 40 µm | 352.5 µm | $\sim 6.3\times10^{-8}$ (Cs) |
| SiC | 35 µm | 387.5 µm | $\sim 5.5\times10^{-14}$ (Cs) |
| OPyC | 40 µm | 427.5 µm | $\sim 6.3\times10^{-8}$ (Cs) |

The buffer's characteristic crossing time is the mean first-passage time across
its thickness $\ell = 100\ \text{µm} = 1\times10^{-4}\ \text{m}$:

$$\tau \approx \frac{\ell^2}{6 D} = \frac{(1\times10^{-4})^2}{6 \times 1\times10^{-8}} \approx 0.17\ \text{s}.$$

So an atom random-walks across the entire buffer in a fraction of a second. Now
compare the single-Gaussian step size at the timesteps the simulator uses:

- $\Delta t = 1\ \text{s}$: $\sigma = \sqrt{2 \cdot 10^{-8} \cdot 1} \approx 1.4\times10^{-4}\ \text{m} = 141\ \text{µm}$ — already **larger than the 100 µm buffer**.
- $\Delta t = 10\ \text{s}$: $\sigma = \sqrt{2 \cdot 10^{-8} \cdot 10} \approx 4.5\times10^{-4}\ \text{m} = 447\ \text{µm}$ — **larger than the whole particle radius (427.5 µm)**.

This matches the empirical note already in the code
(`interaction_with_decaying_nuclide_simulator/mod.rs`): "10 seconds is too high
for the fuel kernel, IPyC, and buffer layer; 1 second is okay for fuel kernel and
IPyC but not buffer layer." A single Gaussian step of $\sigma = 141$–$447$ µm
starting inside the buffer lands the atom past the IPyC and onto (or through) the
SiC in one shot, skipping the IPyC/buffer interface entirely. The interface
transmission/reflection that makes SiC a barrier never gets a chance to act.

## Why the existing mitigations are unsatisfying

- **`scatter_within_triso_particle_gaussian`** samples one Gaussian "velocity"
  (a 1-second Gaussian displacement reinterpreted as a constant velocity) and
  ray-traces in a straight line to the next sphere boundary. That respects the
  geometry, but it replaces diffusion with *ballistic* motion between boundaries:
  the direction persists over the whole sub-flight, so the mean-squared
  displacement and, crucially, the first-passage-time statistics are wrong. It
  does not converge to the diffusion Green's function.
- **Fourier-number sub-timestepping**
  (`move_single_decaying_particle_within_triso_based_on_fourier_no`) caps the
  step at $\mathrm{Fo} = D\,\Delta t/\ell^2 \le 10^{-2}$. This *is* accurate, but
  in the buffer it forces $\Delta t \lesssim 10^{-2}\,\ell^2/D \approx 1.7$ ms, so
  a one-second physical interval needs hundreds of sub-steps — and every atom
  that wanders into the buffer pays that cost. This is the "very slow when too
  many particles end up in the buffer" behaviour flagged in the code comments.

## The fix: Walk-on-Spheres / Green's-function first passage

Rather than discretising time and approximating the bounded propagator by a
free-space Gaussian, sample the **exact** bounded propagator one interface-free
sphere at a time:

1. At the walker's position, let $R$ be the distance to the nearest interface
   (the inner or outer sphere of the current shell). The largest sphere of
   radius $R$ centred on the walker contains no interface, so inside it the
   medium is homogeneous and the free-space theory *is* exact.
2. Advance the walker to a point drawn **uniformly** on that sphere's surface
   (isotropy of Brownian motion from the centre), and add a first-passage
   **time** $\tau$ sampled from the 3-D Brownian exit-time distribution for a
   sphere of radius $R$, whose mean is $\mathbb{E}[\tau] = R^2/(6D)$.
3. Repeat. Hops are large in the bulk (few steps) and shrink automatically as
   the walker nears an interface (many small but still *exact* steps). When the
   walker is within a small $\varepsilon$ of an interface, resolve it with the
   transmission/reflection rule instead of stepping across blindly.

Because a hop never crosses an interface — it lands *on* the nearest one — the
buffer overshoot is impossible by construction, and there is no global timestep
to tune. This is the method implemented under
`lagrangian_diffusion/first_passage/`.

## References

- J. Crank, *The Mathematics of Diffusion*, 2nd ed., Clarendon Press, 1975 —
  bounded-sphere release solution (the single-region verification target).
- W. Jiang, A. Toptan, J. D. Hales, B. W. Spencer, S. R. Novascone, *Fission
  product transport in TRISO particles and pebbles*, INL/EXT-21-63549-Rev001,
  Idaho National Laboratory, 2023 — per-layer diffusion coefficients used above.
- J. D. Hales, W. Jiang, A. Toptan, K. A. Gamble, *Modeling fission product
  diffusion in TRISO fuel particles with BISON*, J. Nucl. Mater. 548 (2021)
  152840 — continuity-of-concentration-and-flux interface treatment.
