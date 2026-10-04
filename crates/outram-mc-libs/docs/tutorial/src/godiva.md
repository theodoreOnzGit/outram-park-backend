# Godiva: a sphere of uranium, one neutron at a time

> **Research, education and V&V only.** Not for reactor operation, licensing,
> safety decisions or emergency response
> ([`RESPONSIBLE_USE.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/RESPONSIBLE_USE.md)).
> **Review status: first draft, 2026-10-04, AI-assisted, not yet reviewed by a
> human.** Built from
> [`@@COMMIT_SHORT@@`](https://github.com/theodoreOnzGit/outram-park-backend/commit/@@COMMIT@@)
> on @@BUILD_DATE@@; every code link points at that commit.

<div class="mcw-demo" data-mc-widget="demo" data-src="../../demos/monte-carlo/?rung=godiva&amp;mode=watch" data-label="▶ Start the Godiva demo here (Watch mode)"></div>

*The demo downloads real ENDF/B-VIII.0 nuclear data and processes it in your
browser before the first neutron flies: allow a minute or two. Each track is a
real history from the transport code; the way they are chained one after
another is an illustration.*

## The problem

At Los Alamos, a bare sphere of highly enriched uranium metal, about the size
of a grapefruit, was assembled until it was **exactly critical**: the chain
reaction neither grew nor died away. It is catalogued as the ICSBEP benchmark
**HEU-MET-FAST-001**, "Godiva", and the evaluation reduces it to a single
homogeneous sphere of radius **8.7407 cm** whose multiplication factor is
**k = 1.0000 ± 0.0010**.

> *History placeholder: a sourced account of the Godiva assembly (who built it,
> when, how it was made critical) is waiting for a source with page numbers,
> to be supplied by the maintainer. No unsourced history is written here.*

**The question of this lesson:** *from nuclear data alone, and nothing fitted,
can a computer predict that this size is critical?*

The model the code runs is these few lines, and nothing else:

```rust,ignore
{{#include ../../../src/vv.rs:model}}
```

([`vv::godiva`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/vv.rs#@@L:crates/outram-mc-libs/src/vv.rs:anchor=model@@);
the same constants the recorded result was measured on.)

---

## 1. What does a neutron do in uranium?

**Answer.** It flies in a straight line until it hits a nucleus. Then one of
four things happens: it bounces off (**elastic** scattering), it bounces off
and leaves the nucleus excited, losing energy (**inelastic**), it is swallowed
(**capture**), or it splits the nucleus (**fission**), which releases two or
three new neutrons. Or, before it hits anything, it reaches the surface and
**leaks** out.

So to follow one neutron we need two things: **how far** it flies, and **what
happens** when it stops. Monte Carlo answers both with random numbers, one
neutron at a time, exactly as nature would (this is *analog* Monte Carlo: no
weights; every tally is a count).

<div class="predict">

**Predict.** A fast neutron in uranium metal: will it typically fly a
millimetre, a few centimetres, or a metre before it hits a nucleus?

</div>

## 2. How far? Sampling the free path

**Answer.** The probability of flying a distance $d$ without a collision is
$e^{-\Sigma_t d}$, where $\Sigma_t$ is the **macroscopic total cross section**,
the probability of a collision per centimetre. To sample a flight, draw a
uniform random number $\xi$ in $[0, 1)$ and solve:

$$d = -\frac{\ln \xi}{\Sigma_t}$$

The mean flight is $1/\Sigma_t$, the **mean free path**. For Godiva's uranium
at 1 MeV, $\Sigma_t = 0.332$ cm⁻¹: a mean free path of **3.0 cm**, a third of
the sphere's radius (measured from the processed ENDF/B-VIII.0 data, see the
table under step 4).

<div class="mcw" data-mc-widget="flights" data-sigma="0.332"></div>

*Illustration (JavaScript's own random numbers). Drag $\Sigma_t$: denser or
more absorbing material, shorter flights.*

**Where does $\Sigma_t$ come from?** $\Sigma_t = \sum_i N_i \sigma_{t,i}$:
the number density of each nuclide times its microscopic cross section, read
from evaluated nuclear data. That is its own track (the nuclear data track,
[#515](https://github.com/theodoreOnzGit/outram-park-backend/issues/515)).
**Where does $\xi$ come from?** From `prn`, a 64-bit linear congruential
generator whose output goes through the **PCG-RXS-M-XS permutation**, as in
OpenMC ([`petir/src/rng/lcg.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/petir/src/rng/lcg.rs#@@L:crates/petir/src/rng/lcg.rs:fn=prn@@)).

**The code walk.** How the program gets from the example you can run to the
line that samples the flight:

<!-- code-walk: from=crates/outram-mc-libs/examples/godiva_keff_endf_local.rs::main to=crates/outram-mc-libs/src/physics/keff.rs::transport_history -->
<div class="codewalk">

- [`main`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/godiva_keff_endf_local.rs#@@L:crates/outram-mc-libs/examples/godiva_keff_endf_local.rs:fn=main@@) in `examples/godiva_keff_endf_local.rs` calls `run_keff(radius_cm, &material, &nuclides, &settings)` *(filled by hand: the example sits behind the `endf-pebble-cases` feature)*
  - [`run_keff`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/keff.rs#@@L:crates/outram-mc-libs/src/physics/keff.rs:fn=run_keff@@) dispatches on `settings.compute`; the default, `CpuSingleThread`, goes to
    - [`run_keff_cpu_single`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/keff.rs#@@L:crates/outram-mc-libs/src/physics/keff.rs:fn=run_keff_cpu_single@@), which is `PowerIteration::new`, then `step` until it returns `None`, then `result`
      - [`PowerIteration::step`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/keff.rs#@@L:crates/outram-mc-libs/src/physics/keff.rs:after=impl+PowerIteration;fn=step@@) calls, for every neutron of the generation,
        - [`transport_history`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/keff.rs#@@L:crates/outram-mc-libs/src/physics/keff.rs:fn=transport_history@@), which samples the flight:
          - [`Material::macro_xs_total_urr`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/material.rs#@@L:crates/outram-mc-libs/src/material/material.rs:fn=macro_xs_total_urr@@) for $\Sigma_t$ at the neutron's energy,
          - [`prn`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/petir/src/rng/lcg.rs#@@L:crates/petir/src/rng/lcg.rs:fn=prn@@) for $\xi$.

</div>
<!-- /code-walk -->

*Generated with `kopitiam callees --lsp` (rust-analyzer) on 2026-10-04, one
hop at a time; the first hop was read from the source.* These are the lines,
pulled from the source when this page was built:

```rust,ignore
{{#include ../../../src/physics/keff.rs:free_flight}}
```

([`keff.rs`, the flight](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/keff.rs#@@L:crates/outram-mc-libs/src/physics/keff.rs:anchor=free_flight@@).)
`-prn(seed).r_ln() / sigma_t` is $d = -\ln\xi / \Sigma_t$. The next two lines
are the next question.

<div class="predict">

**Predict.** A neutron starts at the centre of the sphere (radius 8.74 cm, mean
free path 3 cm). What fraction of first flights reach the surface without a
collision: about 5 %, 30 % or 60 %? And from 1 cm under the surface?

</div>

## 3. The sphere has an edge: surface tracking

**Answer.** Each flight is compared with the distance to the surface along the
same direction, $d_\text{bound}$. **The shorter one wins.** If the collision
distance is shorter, the neutron collides inside. If the surface is nearer,
the neutron crosses it, and since Godiva has nothing outside (a vacuum
boundary), it has **leaked**: it is gone for good.

For a sphere of radius $R$ centred at the origin, a neutron at $\vec{r}$
flying along $\vec{u}$ reaches the surface where $|\vec{r} + d\vec{u}| = R$,
a quadratic in $d$:

$$d^2 + 2 k d + c = 0, \quad k = \vec{r} \cdot \vec{u}, \quad c = |\vec{r}|^2 - R^2$$

$$d_\text{bound} = -k + \sqrt{k^2 - c} \quad (\text{inside the sphere, } c < 0)$$

<div class="mcw" data-mc-widget="surface" data-radius="8.7407" data-sigma="0.332"></div>

*Illustration. From the centre, $e^{-\Sigma_t R} = e^{-0.332 \times 8.74} \approx 0.055$: about 5.5 %
of first flights leak. Move the start towards the edge and watch the fraction
climb. Most of Godiva's leakage happens after a few collisions have carried
the neutron outward.*

**The code walk** continues from `transport_history` into the surface:

<!-- code-walk: from=crates/outram-mc-libs/src/physics/keff.rs::transport_history to=crates/outram-blender/src/csg/surface/quadric.rs::Sphere::distance -->
<div class="codewalk">

- [`transport_history`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/keff.rs#@@L:crates/outram-mc-libs/src/physics/keff.rs:fn=transport_history@@) calls `sphere.distance(r, u, false)`
  - [`Sphere::distance`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-blender/src/csg/surface/quadric.rs#@@L:crates/outram-blender/src/csg/surface/quadric.rs:anchor=sphere_distance@@) in `outram-blender` (the geometry crate), the `Surface` trait's method for a sphere.

</div>
<!-- /code-walk -->

```rust,ignore
{{#include ../../../../outram-blender/src/csg/surface/quadric.rs:sphere_distance}}
```

`k` is $\vec{r}\cdot\vec{u}$, `c` is $|\vec{r}|^2 - R^2$, and the nearer
positive root wins. The `if d_col >= d_bound { … break; }` you saw in step 2
is the whole of surface tracking for a bare sphere: the shorter distance wins,
and reaching the surface first is a leak.

<div class="predict">

**Predict.** When a 1 MeV neutron collides in Godiva, which is most likely:
fission, capture, inelastic or elastic scattering?

</div>

## 4. Which reaction?

**Answer.** Each reaction has its own cross section, and they add up to the
total. Draw a second random number, scale it by the total, $\xi \Sigma_t$, and
see which reaction's share of the bar it lands in. The bigger the cross
section, the more often that reaction is chosen. Counting how often each one
happens is the simplest possible tally.

<div class="mcw" data-mc-widget="reaction" data-table='[{"label":"10 keV","st":0.75183,"f":[0.1738,0.0694,0.0000,0.7568]},{"label":"100 keV","st":0.58170,"f":[0.1228,0.0334,0.0276,0.8162]},{"label":"500 keV","st":0.40284,"f":[0.1282,0.0180,0.1818,0.6719]},{"label":"1 MeV","st":0.33172,"f":[0.1649,0.0157,0.2559,0.5635]},{"label":"2 MeV","st":0.34781,"f":[0.1727,0.0082,0.2992,0.5199]},{"label":"5 MeV","st":0.36636,"f":[0.1373,0.0007,0.2979,0.5641]}]'></div>

*The bar is Godiva's own uranium, from ENDF/B-VIII.0 processed by the
workspace's NJOY port at tolerance 0.001 and 293.6 K, printed by
`cargo run --release -p dhoby-ghaut --example monte_carlo_web -- --xs-table`
on 2026-10-04 (inelastic includes (n,2n), (n,3n) and (n,anything)). At 1 MeV:
fission 16.5 %, capture 1.6 %, inelastic 25.6 %, elastic 56.4 % of
$\Sigma_t = 0.332$ cm⁻¹.*

The real code picks the nuclide first (U-235, U-238 or U-234, in proportion to
each one's $N\sigma_t$), then partitions that nuclide's total:

<!-- code-walk: from=crates/outram-mc-libs/src/physics/keff.rs::transport_history to=crates/outram-mc-libs/src/physics/keff.rs::transport_history (reaction choice) -->
<div class="codewalk">

- [`transport_history`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/keff.rs#@@L:crates/outram-mc-libs/src/physics/keff.rs:fn=transport_history@@) moves the neutron to the collision site, then
  - [`Material::sample_nuclide_urr`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/material.rs#@@L:crates/outram-mc-libs/src/material/material.rs:fn=sample_nuclide_urr@@) chooses the nucleus that was hit,
  - [`Nuclide::xs_at_energy`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#@@L:crates/outram-mc-libs/src/material/nuclide.rs:fn=xs_at_energy@@) gives its cross sections at this energy,
  - and the partition below chooses the reaction.

</div>
<!-- /code-walk -->

```rust,ignore
{{#include ../../../src/physics/keff.rs:reaction_choice}}
```

([`keff.rs`, the reaction choice](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/keff.rs#@@L:crates/outram-mc-libs/src/physics/keff.rs:anchor=reaction_choice@@).)
`xi < x.fission` is the fission part of the bar; `xi < x.absorption` (capture
plus fission) the capture part; the scattering branches follow it in the
source.

## 5. Scattering: how a neutron loses energy

**Answer, short.** In an **elastic** collision with a uranium nucleus, 236
times heavier than the neutron, the neutron bounces off almost like a ball off
a wall: it keeps nearly all its energy (at most about 1.7 % is lost), and the
evaluated data say it mostly carries on **forward**. In an **inelastic**
collision it leaves the nucleus excited and loses a definite chunk of energy,
often hundreds of keV. In uranium, inelastic scattering is how a fast neutron
slows down.

<!-- code-walk: from=crates/outram-mc-libs/src/physics/keff.rs::transport_history to=scattering laws -->
<div class="codewalk">

- [`transport_history`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/keff.rs#@@L:crates/outram-mc-libs/src/physics/keff.rs:fn=transport_history@@), inelastic branch:
  - [`Nuclide::sample_inelastic`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#@@L:crates/outram-mc-libs/src/material/nuclide.rs:fn=sample_inelastic@@) picks the level (or the continuum), then
  - [`two_body_scatter_with_mu`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/scatter.rs#@@L:crates/outram-mc-libs/src/physics/scatter.rs:fn=two_body_scatter_with_mu@@) applies the kinematics, with the level's own angular law from
  - [`Nuclide::sample_inelastic_mu_cm`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#@@L:crates/outram-mc-libs/src/material/nuclide.rs:fn=sample_inelastic_mu_cm@@).
- elastic branch (the last `else`):
  - [`Nuclide::sample_elastic_mu_cm`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#@@L:crates/outram-mc-libs/src/material/nuclide.rs:fn=sample_elastic_mu_cm@@) samples the evaluated (ENDF MF=4) angle,
  - [`free_gas_elastic_scatter_dbrc`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/scatter.rs#@@L:crates/outram-mc-libs/src/physics/scatter.rs:fn=free_gas_elastic_scatter_dbrc@@) applies it. Above 400 kT (all of Godiva's spectrum) the target is effectively at rest; below, its thermal motion is sampled. That fork matters in the next rung, not here.

</div>
<!-- /code-walk -->

```rust,ignore
{{#include ../../../src/physics/keff.rs:inelastic}}
```

*Go deeper:* the deep dive's
[collisions chapter](../../deep-dives/monte-carlo/collisions-and-fission.html).
Why "forward" matters so much is in the history box under step 8.

## 6. Fission makes the next generation

**Answer.** A fission releases $\bar\nu \approx 2.6$ neutrons on average (the
evaluated $\bar\nu(E)$ of the nucleus that split). The code does not follow
them at once: it writes down where they are born, in a **fission bank**, and
finishes the neutron it is following. When every neutron of a generation has
been followed, the bank becomes the next generation. The multiplication
factor is the ratio of the two generations:

$$k = \frac{\text{neutrons born in generation } n+1}{\text{neutrons in generation } n}$$

In the code, each generation starts with exactly $N$ neutrons, and $k$ is the
total $\bar\nu$ of its fissions divided by $N$.

<!-- code-walk: from=crates/outram-mc-libs/src/physics/keff.rs::PowerIteration::step to=crates/outram-mc-libs/src/physics/fission.rs::comb_resample -->
<div class="codewalk">

- [`PowerIteration::step`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/keff.rs#@@L:crates/outram-mc-libs/src/physics/keff.rs:after=impl+PowerIteration;fn=step@@)
  - [`transport_history`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/keff.rs#@@L:crates/outram-mc-libs/src/physics/keff.rs:fn=transport_history@@) adds $\bar\nu$ to `production` and pushes the new neutrons to `next_bank`:
    - [`sample_num_neutrons`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/fission.rs#@@L:crates/outram-mc-libs/src/physics/fission.rs:fn=sample_num_neutrons@@) decides how many sites to bank, $\lfloor \bar\nu/k \rfloor$ or one more,
    - [`Nuclide::sample_fission_energy_below`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#@@L:crates/outram-mc-libs/src/material/nuclide.rs:fn=sample_fission_energy_below@@) draws each one's energy from the evaluated fission spectrum;
  - `resample` → [`comb_resample`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/fission.rs#@@L:crates/outram-mc-libs/src/physics/fission.rs:fn=comb_resample@@) brings the bank back to exactly $N$ sites, as OpenMC's `synchronize_bank` does;
  - [`RegularMeshExt::shannon_entropy`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/tally/mesh.rs#@@L:crates/outram-mc-libs/src/tally/mesh.rs:after=impl+RegularMeshExt+for+RegularMesh;fn=shannon_entropy@@) measures how spread out the bank is (when a mesh is given).

</div>
<!-- /code-walk -->

```rust,ignore
{{#include ../../../src/physics/keff.rs:fission_bank}}
```

and the generation itself, in
[`PowerIteration::step`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/keff.rs#@@L:crates/outram-mc-libs/src/physics/keff.rs:anchor=generation@@):

```rust,ignore
{{#include ../../../src/physics/keff.rs:generation}}
```

**Why throw generations away?** The first generation has to start from a
guess. Its $k$ describes the guess, not Godiva, so the first generations are
**inactive**: run, then discarded. How do we know when the source has
forgotten its start? The **Shannon entropy** of the fission bank (how spread
out the fission sites are, in bits) rises from a concentrated guess and
**flattens** once the shape has settled.

<div class="mcw-demo" data-mc-widget="demo" data-src="../../demos/monte-carlo/?rung=godiva&amp;mode=watch" data-label="▶ Watch generations spread (demo, then choose “whole generations”)"></div>

*In the demo's Watch mode, choose "whole generations": a real power iteration,
1000 neutrons per generation, all started at the centre. The dots spread to
fill the sphere and the entropy curve climbs and levels off.*

**A guess that is worse than it looks.** The ordinary start
([`PowerIteration::new`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/keff.rs#@@L:crates/outram-mc-libs/src/physics/keff.rs:anchor=initial_source@@))
places the neutrons uniformly in the sphere, but it aims every one of them
straight **outward**, because the same random direction is used for both. So
generation 1 leaks far too much: in the run recorded below its $k$ is
**0.468**, and from generation 2 on it is near 1. That is a defect in the
starting guess (filed as
[#527](https://github.com/theodoreOnzGit/outram-park-backend/issues/527)),
and it is exactly what inactive generations are for: they forget it.

<div class="predict">

**Predict.** Of every 100 neutrons in a generation, how many leak out, how
many are captured, and how many cause fission? (Hint: for $k = 1$, the
fissions must make about 100 new neutrons, at about 2.6 each.)

</div>

## 7. Where do the neutrons go? Leakage and fast fission, by counting

**Answer.** Every neutron ends **exactly one way**: it leaks, it is captured,
or it causes a fission. So the three counts add up to the number of neutrons
followed, and two ratios fall out of them with no extra physics:

- the **non-leakage probability**, $P_{NL} = 1 - L/N$;
- the multiplication of an **infinite** block of the same uranium, where
  nothing can leak: $k_\infty = \bar\nu F / (F + C)$.

Then $k = \bar\nu F / N = k_\infty P_{NL}$: the surface costs Godiva its
leakage, and nothing else.

The counts are tallied by the transport itself, one increment where each
neutron ends
([`HistoryCounts`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/keff.rs#@@L:crates/outram-mc-libs/src/physics/keff.rs:struct=HistoryCounts@@);
the `counts.` lines in the code above). **Recorded run** for this page,
2026-10-04, natively, the record's settings and seed 1:
`cargo run --release -p dhoby-ghaut --example monte_carlo_web -- --headless-keff 5000 40 120 1`
(the same output the demo's Run k_eff mode prints), over the 120 active
generations:

| | count | share |
|---|---|---|
| neutrons followed | 601 638 | 600 000 source + 1 638 from (n,2n)-type reactions |
| leaked | 343 884 | 57.2 % |
| captured | 26 806 | 4.5 % |
| fissioned | 230 948 | 38.4 % |

$\bar\nu = 2.5965$, $k_\infty = 2.32645$, $P_{NL} = 0.42842$, so
$k_\infty P_{NL} = 0.99670$, against $k = \bar\nu F / N_\text{source} = 0.99942$.
**The two differ by 0.27 %, and that is not a mistake:** $(n,2n)$ and similar
reactions add 1 638 neutrons (0.27 %) that were never source neutrons, so
dividing by the neutrons *followed* rather than the neutrons *started* drops
exactly them. **More than half of all neutrons leak.** A bigger sphere
leaks less, which is why there is a critical size; that is the exercise at the
end.

**Is Godiva's fission fast?** The same tally bins each fission by the energy
of the neutron that caused it:

| incident energy | fissions | share |
|---|---|---|
| below 0.625 eV (thermal) | 0 | 0 % |
| 0.625 eV to 100 keV | 11 068 | 4.8 % |
| above 100 keV | 219 880 | 95.2 % |

**Not one thermal fission.** Godiva has no moderator: nothing light enough to
slow a neutron down before it leaks or fissions. That is why **no "fast
fission factor" $\varepsilon$ is quoted for Godiva**: $\varepsilon$ is total
over thermal fissions, undefined when thermal fission is zero. In a thermal
reactor, fast fission is a small bonus on top of thermal fission, and that
bonus is $\varepsilon$; it is measured in the next rung, uranium mixed into
graphite.

The demo's **Run k_eff** mode shows these counts live, for your own run.

## 8. Is Godiva critical?

<div class="mcw-demo" data-mc-widget="demo" data-src="../../demos/monte-carlo/?rung=godiva&amp;mode=run" data-label="▶ Run k_eff yourself (demo, Run mode)"></div>

*Run k_eff processes the data at NJOY's full tolerance (0.001), like the
record below, so expect it to load more slowly than Watch mode. Then press
▶ Run k_eff: 5000 neutrons × [40 inactive + 120 active] generations, one
console line per generation, as `openmc.run()` prints.*

**The result to quote.** Five ways of computing Godiva were compared in
[`five_route_keff_2026_09_29.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/verification_and_validation/icsbep/five_route_keff_2026_09_29.md),
section "Results — after the OpenMC-parity audit" (2026-09-30, commit
`0414bc8277`). The one this lesson's code is, **route 4: `outram-mc-libs`
reading ENDF/B-VIII.0 directly**, 32 independent seeds × 5000 neutrons ×
[40 + 120] generations:

$$k_\text{eff} = 0.99948 \pm 0.00027 \quad (-52 \pm 27 \text{ pcm})$$

against the experiment, **1.0000 ± 0.0010** (HEU-MET-FAST-001). One pcm is
$10^{-5}$ in $k$.

- **What the ± means.** Each seed is a complete, independent run. The 32
  results scatter with a standard deviation of **151 pcm**; the mean of 32 is
  known to $151/\sqrt{32} = 27$ pcm. *Why that is trustworthy is a later
  lesson.*
- **Validation.** Against the experiment, $-52 \pm 27$ pcm is well inside the
  experiment's own ±100 pcm. That is a check against a measurement: a
  **validation**.
- **Verification.** Beside it, **OpenMC** reading NJOY2016 data (route 1 of
  the same record; *route 1 is OpenMC, not this code*) gives
  1.00016 ± 0.00021, **+16 ± 21 pcm**. The difference, **−68 ± 34 pcm
  (2.0 σ)**, is shown as it is: two codes, two data paths, agreeing to within
  a tenth of a percent but not yet within their statistics.
- **Your run.** One run at the default settings is one seed of the 32, so
  expect it to land within about ±151 pcm of the record, and its own ± (the
  spread of its 120 generations) to be of that size too. A run done natively
  for this page (seed 1, 2026-10-04, i9-13900K, one thread) gave
  **0.99942 ± 0.00183** (−58 ± 183 pcm); its transport took 2.9 s and the
  data processing 62 s.

<div class="history">

**History, not the current result: how the code got here (measured 2026-07-03,
flagged superseded 2026-08-06).** The same Godiva model, each line adding one
piece of physics, from the record in
[`examples/godiva_keff_endf.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/godiva_keff_endf.rs#@@L:crates/outram-mc-libs/examples/godiva_keff_endf.rs:text=HIGH:+CE+data+only,+elastic-lumped@@)
(ENDF/B-VII.1, 5000 × [40 + 120], single seeds, before the random-number
generator change `op-jis`; the file flags every value as superseded and none
has been re-run):

| run | k_eff | from the benchmark |
|---|---|---|
| continuous-energy data, every scatter treated as elastic | 1.12451 ± 0.00202 | +12 451 pcm |
| + inelastic energy loss | 1.09942 ± 0.00169 | +9 942 pcm |
| + anisotropic (forward-peaked) elastic, ENDF MF=4 | 0.99701 ± 0.00168 | −299 pcm |
| + (n,2n) makes two neutrons | 0.99872 ± 0.00173 | −128 pcm |
| + energy-dependent fission spectrum, ENDF MF=5 | 1.00367 ± 0.00182 | +367 pcm |

The lesson in it: for a bare fast sphere, **forward-peaked elastic
scattering** was worth about 10 000 pcm, because a neutron that keeps going
forward reaches the surface sooner. The data's fidelity mattered far less
than the physics of the collision. The current number is the one above,
−52 ± 27 pcm.

</div>

**Run it yourself.** On your own machine, from a clone of the repository:

```text
cargo run --release -p outram-mc-libs --features endf-pebble-cases --example godiva_keff_endf_local
```

It processes the three tapes from `reference-data/endf/`, runs 5000 × [40 +
120] generations and prints $k$ against the benchmark.

**Modify.** Change the radius: the line `let radius_cm = godiva::RADIUS_CM;`
in [`godiva_keff_endf_local.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/godiva_keff_endf_local.rs#@@L:crates/outram-mc-libs/examples/godiva_keff_endf_local.rs:fn=main@@).
Try 8 cm and 9.5 cm first and predict which way $k$ moves. (The example's own
check compares against the benchmark, so it will fail for any radius but
Godiva's: that is the check working.)

**Create.** Find the **critical radius**, where $k = 1$, from the data alone:
bisect between a radius that is subcritical and one that is supercritical.
How close to 8.7407 cm do you get, and how would you put an error bar on your
radius from the error bar on $k$?

## Next: slowing down

Godiva never slows its neutrons down: not one thermal fission. Mix the uranium
into graphite and almost every fission becomes thermal, the fast fission
factor $\varepsilon$ becomes measurable, and four new questions open (how do
neutrons slow down, what is a resonance, why do neutrons escape it, and why
was the 1942 pile built from lumps). That is **rung 2**, uranium in graphite
([#524](https://github.com/theodoreOnzGit/outram-park-backend/issues/524)).

## The whole call tree

Everything the Run k_eff path reaches inside the workspace, from `run_keff`
down to the functions each lesson step stopped at. Generated hop by hop with
`kopitiam callees --lsp` on 2026-10-04 (std and dependency calls removed;
`run_keff`'s other backends, `run_keff_cpu_multi` and `run_keff_gpu`, not
expanded). Calls *inside* the nuclear-data and scattering functions are not
expanded here; the generated tree (#523) will.

<!-- code-walk-tree: from=crates/outram-mc-libs/src/physics/keff.rs::run_keff -->
<div class="codewalk">

- `run_keff` → `run_keff_cpu_single` (default) · `run_keff_cpu_multi` · `run_keff_gpu`
  - `run_keff_cpu_single` → `PowerIteration::new`, `PowerIteration::step`, `PowerIteration::result`
    - `PowerIteration::step` → `transport_history`, `RegularMeshExt::shannon_entropy`, `resample`, `raffles::estimators::mean_and_stderr`
      - `transport_history` → `library_energy_max_ev`, `future_seed`, `Material::macro_xs_total_urr`, `prn`, `r_ln`, `Sphere::distance`, `stream`, `Material::sample_nuclide_urr`, `Nuclide::needs_urr_draw`, `Nuclide::xs_at_energy_urr`, `urr_xi`, `Nuclide::xs_at_energy`, `sample_num_neutrons`, `isotropic_direction`, `Direction::new`, `Nuclide::sample_fission_energy_below`, `HistoryCounts::record_fission`, `Nuclide::sample_inelastic`, `Nuclide::sample_inelastic_mu_cm`, `two_body_scatter_with_mu`, `two_body_scatter`, `Nuclide::sample_inelastic_emission`, `Nuclide::has_evaluated_emission`, `Nuclide::emits_n2n_secondary`, `Nuclide::sample_mt5_multiplicity`, `Nuclide::sample_other_emission`, `Nuclide::sample_thermal`, `rotate_direction`, `Nuclide::free_gas_kt`, `Nuclide::sample_elastic_mu_cm`, `free_gas_elastic_scatter_dbrc`, `Nuclide::dbrc_table`
      - `resample` → `comb_resample` *(read from the source)*

</div>

In the demo, the browser's worker reaches `PowerIteration::step` through
[`godiva::sim::Keff::step`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/dhoby-ghaut/examples/monte_carlo_web/godiva/sim.rs#@@L:crates/dhoby-ghaut/examples/monte_carlo_web/godiva/sim.rs:after=impl+Keff;fn=step@@),
called from
[`Loaded::serve`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/dhoby-ghaut/examples/monte_carlo_web/engine.rs#@@L:crates/dhoby-ghaut/examples/monte_carlo_web/engine.rs:fn=serve@@)
when the page asks for the next generation *(filled by hand: page and worker
talk by messages, not calls)*.

---

**Deliberate liberties.** The model is the benchmark's own simplified model:
one homogeneous sphere, three uranium isotopes, 293.6 K. The demo's Watch mode
processes the data at the loosened tolerance 0.01, whose measured effect on
Godiva is +7 ± 41 pcm (`outram-mc-libs/docs/profiling/speed_tiers_2026_09_27.md`);
Run k_eff uses NJOY's 0.001. The initial source aims every neutron outward
(#527), which the inactive generations forget.

**Literature.** ICSBEP, *International Handbook of Evaluated Criticality
Safety Benchmark Experiments*, evaluation HEU-MET-FAST-001 (Godiva). The
handbook is licence-restricted (`DATA_POLICY.md`); only its published model
and benchmark value are used here.

**Doesn't tally?** If anything here disagrees with the code it links to, the
page is wrong:
[report it](https://github.com/theodoreOnzGit/outram-park-backend/issues/new?title=Godiva%20lesson%20doesn%27t%20tally%3A%20&labels=bug).
This page changes whenever `develop` does; it was built from
`@@COMMIT_SHORT@@` on @@BUILD_DATE@@. Tracking issue
[#521](https://github.com/theodoreOnzGit/outram-park-backend/issues/521).
