# The legacy Gaussian engine

> **Research, education and V&V only** ([`RESPONSIBLE_USE.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/RESPONSIBLE_USE.md)).
> **Review status:** AI-assisted draft, 2026-10-04, not yet human-reviewed.

**Extended deep dive.** [Rung 3](../../tutorials/triso-atops/walk.html) told why the first diffusion
engine was replaced. The code is still in the crate, beside Walk-on-Spheres,
which the crate calls "the intended replacement for the diffusion core"
([`first_passage/mod.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/lagrangian_decay_simulator/lagrangian_diffusion/first_passage/mod.rs#L33-L35)).
This page maps it, so a reader can tell which path a piece of code is on.

## The map

| Module | What it does | Built when |
|---|---|---|
| [`single_particle_simulator`](../../api/boon_lay/lagrangian_decay_simulator/lagrangian_diffusion/single_particle_simulator/index.html) (`SingleParticleDiffusionSimulatorMC`) | one atom; isotropic scattering with a macroscopic cross section, or a Gaussian velocity per step | always |
| `…::movement_within_triso_particle` | Gaussian steps inside a `TrisoCell` (four variants: Gaussian, simple Gaussian, coupled to a decaying nuclide, brute force) | always |
| `…::interaction_with_decaying_nuclide_simulator` | couples a moving atom to rung 2's `SingleNuclideSimulatorMC` (isotropic, Gaussian with collision counts, Fourier-number time steps) | always |
| `…::cached_normals` (`DiffusionRandomCache`) | pre-drawn standard normals to speed up the Gaussian steps | always |
| `…::constructive_solid_geometry::chatgpt_vibe_coded_sphere_crossing` | time for a straight-line velocity to first cross a sphere (used by the Gaussian step's boundary check) | always |
| [`central_limit_theorem`](../../api/boon_lay/lagrangian_decay_simulator/lagrangian_diffusion/central_limit_theorem/index.html) | per-component variance $\sigma^2 = nE[S^2]/3$ from $n$ isotropic steps; $E[S^2] = 2\ell^2$ for exponential steps of mean $\ell$; the Gaussian sampler; `OoRng64` | always |
| [`isotropic_scattering`](../../api/boon_lay/lagrangian_decay_simulator/lagrangian_diffusion/isotropic_scattering/index.html) | a small `Vec3` for the isotropic random walk | always |
| [`temperature_dependent_collisions`](../../api/boon_lay/lagrangian_decay_simulator/lagrangian_diffusion/temperature_dependent_collisions/index.html) | the Jiang `D(T)` [(Jiang et al., 2023)](#ref-jiang2023fission) used by **both** engines (rung 4), plus `mean_speed` (Maxwell–Boltzmann) and `expected_collisions_atomic_jumps` | always |
| `chatgpt_5_*` (four files) | the first ChatGPT-assisted experiments: a distance-based and a time-based CLT simulator, a vector sampler on a shell | **tests only** (`cfg(test)`) |
| `triso_particle_widget` | an egui drawing of the particle | tests only, and never on Android |

Names starting with `chatgpt_` record honestly where the code came from; the
crate's module docs say which parts were AI-assisted.

## Why it is kept

1. **It is the case study.** Rung 3's lesson, *the central limit theorem gives
   the free-space answer and a bounded domain is not free space*, is made
   concrete by this code and by
   [`docs/buffer_clt_failure_analysis.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/docs/buffer_clt_failure_analysis.md).
2. **An example still uses it.** `triso_simulator`'s backend
   (`examples/triso_simulator/triso_simulator_v1/backend/run.rs`) drives
   `SingleParticleDiffusionSimulatorMC`; it predates Walk-on-Spheres.
3. **The coefficient table is shared.** `temperature_dependent_collisions` is
   not legacy at all; the new engine calls it on every hop.

## The checks

The Gaussian path has **no V&V record**. Outside the `cfg(test)`
experiments, one unit test exercises it
(`interaction_with_decaying_nuclide_simulator/tests_for_auto_timestepping.rs`);
`central_limit_theorem`, `cached_normals` and `SingleParticleDiffusionSimulatorMC`
have none of their own (counted 2026-10-04). Nothing compares a release from
this path with an exact solution, and the buffer analysis shows why such a
comparison would fail at practical time steps. Do not quote a release number
from this path.

## Show the forks: where the two engines meet

```text
TrisoCell::try_get_diffusion_coefficient  (rung 1)
   ├── Gaussian path:  scatter_within_triso_particle_gaussian*  -> one Gaussian step per dt
   └── WoS path:       WoSWalker::step_multilayer / hop          -> exact first-passage hops
          both -> try_get_diffusion_coeff_jiang  (rung 4)
```

<!-- references:begin -->
## References

<p class="csl-entry" id="ref-jiang2023fission" style="padding-left: 2em; text-indent: -2em;">Jiang, W., Toptan, A., Hales, J. D., Spencer, B. W., & Novascone, S. R. (2023). <span style="font-style: italic;">Fission product transport in TRISO particles and pebbles</span>. Idaho National Lab.(INL), Idaho Falls, ID (United States).</p>

<!-- references:end -->
