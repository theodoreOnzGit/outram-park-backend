# Godiva: a sphere of uranium, one neutron at a time

> **Research, education and V&V only.** Not for reactor operation, licensing,
> safety decisions or emergency response
> ([`RESPONSIBLE_USE.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/RESPONSIBLE_USE.md)).
> **Review status: first draft, 2026-10-04, AI-assisted, not yet reviewed by a
> human.** Built from
> [`@@COMMIT_SHORT@@`](https://github.com/theodoreOnzGit/outram-park-backend/commit/@@COMMIT@@)
> on @@BUILD_DATE@@; every code link points at that commit.

<div class="mcw-demo" data-mc-widget="demo" data-src="../../demos/monte-carlo/?rung=godiva&amp;mode=watch" data-label="▶ Start the Godiva demo here (Watch mode)"></div>

*The demo downloads real ENDF/B-VIII.0 [(Brown & others, 2018)](#ref-brown2018endf8) nuclear data and processes it in your
browser before the first neutron flies: allow a minute or two. Each track is a
real history from the transport code; the way they are chained one after
another is an illustration.*

## The problem

At Los Alamos, a bare sphere of highly enriched uranium metal, about the size
of a grapefruit, was assembled until it was **exactly critical**: the chain
reaction neither grew nor died away. It is catalogued as the ICSBEP benchmark
**HEU-MET-FAST-001** [(Briggs et al., 2003)](#ref-briggs2003international), "Godiva", and the evaluation reduces it to a single
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

**In the code.** How the program gets from the example you can run to the
line that samples the flight:

> **On the [Code Review map](../../code-review/):** start at [`main`](../../code-review/#crates/outram-mc-libs/examples/godiva_keff_endf_local.rs::main) and follow its callees (▸) to [`run_keff_cpu_single`](../../code-review/#crates/outram-mc-libs/src/physics/keff.rs::run_keff_cpu_single). Each card opens the function's source and shows its review stamp.

and from the default backend down to the two lines that sample the flight
(`Material::macro_xs_total_urr` gives $\Sigma_t$; `prn`, called on the same line
of `transport_history`, gives $\xi$):

> **On the [Code Review map](../../code-review/):** start at [`run_keff_cpu_single`](../../code-review/#crates/outram-mc-libs/src/physics/keff.rs::run_keff_cpu_single) and follow its callees (▸) to [`Material::macro_xs_total_urr`](../../code-review/#crates/outram-mc-libs/src/material/material.rs::Material::macro_xs_total_urr). Each card opens the function's source and shows its review stamp.

These are the lines, pulled from the source when this page
was built:

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

$$d^2 + 2 k d + c = 0$$

$$k = \vec{r} \cdot \vec{u}, \quad c = |\vec{r}|^2 - R^2$$

Inside the sphere $c < 0$, so one root is positive, and that is the distance
to the surface:

$$d_\text{bound} = -k + \sqrt{k^2 - c}$$

<div class="mcw" data-mc-widget="surface" data-radius="8.7407" data-sigma="0.332"></div>

*Illustration. From the centre, $e^{-\Sigma_t R} = e^{-0.332 \times 8.74} \approx 0.055$: about 5.5 %
of first flights leak. Move the start towards the edge and watch the fraction
climb. Most of Godiva's leakage happens after a few collisions have carried
the neutron outward.*

**In the code**, the path continues from `transport_history` into the surface:

> **On the [Code Review map](../../code-review/):** start at [`transport_history`](../../code-review/#crates/outram-mc-libs/src/physics/keff.rs::transport_history) and follow its callees (▸) to [`Sphere::distance`](../../code-review/#crates/outram-blender/src/csg/surface/quadric.rs::Sphere::distance). Each card opens the function's source and shows its review stamp.

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

> **On the [Code Review map](../../code-review/):** start at [`transport_history`](../../code-review/#crates/outram-mc-libs/src/physics/keff.rs::transport_history) and follow its callees (▸) to [`Nuclide::xs_at_energy`](../../code-review/#crates/outram-mc-libs/src/material/nuclide.rs::Nuclide::xs_at_energy). Each card opens the function's source and shows its review stamp.

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

> **On the [Code Review map](../../code-review/):** start at [`transport_history`](../../code-review/#crates/outram-mc-libs/src/physics/keff.rs::transport_history) and follow its callees (▸) to [`free_gas_elastic_scatter_dbrc`](../../code-review/#crates/outram-mc-libs/src/physics/scatter.rs::free_gas_elastic_scatter_dbrc). Each card opens the function's source and shows its review stamp.

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

$$k = \frac{N_{n+1}}{N_n}$$

where $N_n$ is the number of neutrons in generation $n$ and $N_{n+1}$ the number its fissions produce.

In the code, each generation starts with exactly $N$ neutrons, and $k$ is the
total $\bar\nu$ of its fissions divided by $N$.

> **On the [Code Review map](../../code-review/):** start at [`PowerIteration::step`](../../code-review/#crates/outram-mc-libs/src/physics/keff.rs::PowerIteration::step) and follow its callees (▸) to [`comb_resample`](../../code-review/#crates/outram-mc-libs/src/physics/fission.rs::comb_resample). Each card opens the function's source and shows its review stamp.

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
places the neutrons uniformly in the sphere and aims each one in its own
random direction. In the run recorded below, generation 1 has $k$ =
**0.897** and the later generations scatter around 1. Generation 1 is still
low, most likely because a uniform source puts more neutrons near the surface
than the settled source does (that one is peaked towards the centre), so more
of them leak; that explanation is ours and has not been measured.
Forgetting the starting guess is exactly what inactive generations are for.

~~It aims every one of them straight **outward**, because the same random
direction is used for both; generation 1's $k$ is **0.468**~~ **CORRECTED
2026-10-05**: that was the start until
[#527](https://github.com/theodoreOnzGit/outram-park-backend/issues/527) was
fixed, when one random direction both placed and aimed each neutron. The
fix draws a second, independent direction. Generation 1 went from 0.468 to
0.897 (seed 1). The answer did not move: 512 seeds after the fix give
−7 ± 8 pcm, against −6 ± 5 pcm before.

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
2026-10-05 (re-run after #527 changed the initial source), natively, the
record's settings and seed 1:
`cargo run --release -p dhoby-ghaut --example monte_carlo_web -- --headless-keff 5000 40 120 1`
(the same output the demo's Run k_eff mode prints), over the 120 active
generations:

| | count | share |
|---|---|---|
| neutrons followed | 601 643 | 600 000 source + 1 643 from (n,2n)-type reactions |
| leaked | 344 386 | 57.2 % |
| captured | 26 865 | 4.5 % |
| fissioned | 230 392 | 38.3 % |

$\bar\nu = 2.5962$, $k_\infty = 2.32505$, $P_{NL} = 0.42759$, so
$k_\infty P_{NL} = 0.99417$, against $k = \bar\nu F / N_\text{source} = 0.99689$.
*(The 2026-10-04 run, on the old source, gave 601 638 / 343 884 / 26 806 /
230 948 and $k$ = 0.99942: the same shares, a different draw.)*
**The two differ by 0.27 %, and that is not a mistake:** $(n,2n)$ and similar
reactions add 1 643 neutrons (0.27 %) that were never source neutrons, so
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

**The result to quote.** This lesson's program, run **1024 times with
independent seeds** (`examples/godiva_keff_ensemble.rs`, 5000 neutrons ×
[40 + 120] generations, ENDF/B-VIII.0 read directly; 2026-10-05, commit
`6faff1ed8`, recorded as `RECORDED_PCM` in
[`godiva_keff_endf_local.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/godiva_keff_endf_local.rs),
[#546](https://github.com/theodoreOnzGit/outram-park-backend/issues/546)):

$$k_\text{eff} = 0.99994 \pm 0.00005$$

that is, **−6 ± 5 pcm** from 1, against the experiment's
**1.0000 ± 0.0010** (HEU-MET-FAST-001). One pcm is
$10^{-5}$ in $k$.

~~$k_\text{eff} = 0.99948 \pm 0.00027$, −52 ± 27 pcm (route 4 of the
five-route record, 32 seeds, 2026-09-30, `0414bc8277`)~~: superseded on
2026-10-05. The same route, re-run on the same code, reproduces those 32
seeds digit for digit, and over 288 seeds gives −10 ± 11 pcm. The 32 seeds
had simply come out low (see "Later measurements" in
[`five_route_keff_2026_09_29.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/verification_and_validation/icsbep/five_route_keff_2026_09_29.md)).

- **What the ± means.** Each seed is a complete, independent run. The 1024
  results scatter with a standard deviation of **165 pcm**, so the mean of
  1024 is known to $165/\sqrt{1024} = 5$ pcm. *Why that is trustworthy is a
  later lesson.*
- **Validation.** Against the experiment, $-6 \pm 5$ pcm is well inside the
  experiment's own ±100 pcm. That is a check against a measurement: a
  **validation**. It does not mean "6 pcm accurate": the experiment itself is
  known only to ±100 pcm.
- **Verification.** Beside it, **OpenMC** [(Romano et al., 2015)](#ref-romano2015openmc) reading NJOY2016 [(MacFarlane et al., 2017)](#ref-njoy2016) data (route 1 of
  the five-route record, 32 seeds, 2026-09-30; *route 1 is OpenMC, not this
  code*) gives 1.00016 ± 0.00021, **+16 ± 21 pcm**. The difference is
  **−22 ± 22 pcm (1.0 σ)**: two codes and two data paths agree within their
  statistics. ~~The difference, −68 ± 34 pcm (2.0 σ)~~ was against the low
  32-seed draw above.
- **Your run.** One run at the default settings is one seed of the 1024, so
  expect it to land within about ±165 pcm of the record, and its own ± (the
  spread of its 120 generations) to be of that size too. A run done natively
  for this page (seed 1, 2026-10-05, after #527; Intel Xeon @ 2.10 GHz, one
  thread of a shared 4-core machine) gave **0.99689 ± 0.00178**
  (−311 ± 178 pcm), 1.7 of its own σ from the record; its transport took
  5.6 s and the data processing 99 s. The demo, at its defaults in headless
  Chromium, printed **the same k and the same counts**, digit for digit:
  seed 1 is seed 1, in a browser or not. (~~0.99942 ± 0.00183, −58 ± 183 pcm,
  2026-10-04, i9-13900K~~: the same seed on the old initial source.)

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
−6 ± 5 pcm (~~−52 ± 27 pcm~~ until 2026-10-05).

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
was the 1942 pile built from lumps). That is **[rung 2, uranium in
graphite](ugraphite.md)**
([#524](https://github.com/theodoreOnzGit/outram-park-backend/issues/524)).

## The whole call tree

Everything the reader's run reaches inside the workspace from the default
backend, `run_keff_cpu_single`, as an architecture map, on the [Code Review map](../../code-review/): unfold its callees (▸) level by level.
`run_keff`'s other backends,
`run_keff_cpu_multi` and `run_keff_gpu`, run the same `transport_history`.

> **On the [Code Review map](../../code-review/):** start at [`run_keff_cpu_single`](../../code-review/#crates/outram-mc-libs/src/physics/keff.rs::run_keff_cpu_single). Each card opens the function's source and shows its review stamp.

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

<!-- references:begin -->
## References

<p class="csl-entry" id="ref-briggs2003international" style="padding-left: 2em; text-indent: -2em;">Briggs, J. B., Scott, L., &#38; Nouri, A. (2003). The international criticality safety benchmark evaluation project. <i>Nuclear Science and Engineering</i>, <i>145</i>(1), 1–10.</p>

<p class="csl-entry" id="ref-brown2018endf8" style="padding-left: 2em; text-indent: -2em;">Brown, D. A. &#38; others. (2018). ENDF/B-VIII.0: The 8th Major Release of the Nuclear Reaction Data Library with CIELO-project Cross Sections, New Standards and Thermal Scattering Data. <i>Nuclear Data Sheets</i>, <i>148</i>, 1–142. https://doi.org/10.1016/j.nds.2018.02.001</p>

<p class="csl-entry" id="ref-njoy2016" style="padding-left: 2em; text-indent: -2em;">MacFarlane, R. E., Muir, D. W., Boicourt, R. M., Kahler, A. C., &#38; Conlin, J. L. (2017). <i>The NJOY Nuclear Data Processing System, Version 2016</i> (Technical Report LA-UR-17-20093). Los Alamos National Laboratory. https://www.osti.gov/biblio/1338791</p>

<p class="csl-entry" id="ref-romano2015openmc" style="padding-left: 2em; text-indent: -2em;">Romano, P. K., Horelik, N. E., Herman, B. R., Nelson, A. G., Forget, B., &#38; Smith, K. (2015). OpenMC: A state-of-the-art Monte Carlo code for research and development. <i>Annals of Nuclear Energy</i>, <i>82</i>, 90–97.</p>

<!-- references:end -->
