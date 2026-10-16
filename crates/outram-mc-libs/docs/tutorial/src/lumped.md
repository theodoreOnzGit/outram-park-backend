# Lumping: why the pile put its uranium in lumps

> **Research, education and V&V only.** Not for reactor operation, licensing,
> safety decisions or emergency response
> ([`RESPONSIBLE_USE.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/RESPONSIBLE_USE.md)).
> **Review status: first draft, 2026-10-05, AI-assisted, not yet reviewed by a
> human.** Built from
> [`@@COMMIT_SHORT@@`](https://github.com/theodoreOnzGit/outram-park-backend/commit/@@COMMIT@@)
> on @@BUILD_DATE@@; every code link points at that commit.

<div class="mcw-demo" data-mc-widget="demo" data-src="../../demos/monte-carlo/?rung=lumped&amp;mode=watch" data-label="▶ Start the lumped-cell demo here (Watch mode)"></div>

*The demo processes the same six tapes as rung 2 in your browser (a minute or
two). Each track is a real history in one cell: a sphere of natural uranium
metal in graphite, with a white boundary. Zoom in on the lump and watch where
the neutrons that slow down in the graphite are caught.*

## The problem

[Rung 2](ugraphite.md) mixed natural uranium evenly through graphite and asked
whether any ratio of carbon to uranium could sustain a chain reaction.
It could not: the best ratio, about 400–500 carbon atoms per uranium atom,
gave $k_\infty = 0.7785 \pm 0.0022$ (rung 2, step 7, measured 2026-10-05).
More carbon helps the neutrons past U-238's resonances ($p$ rises) but steals
the thermal neutrons ($f$ falls).

> *History placeholder: the 1942 Chicago pile put its natural uranium (metal
> and oxide) in lumps inside graphite blocks. No source on its design is in
> the literature corpus yet (`crates/kovan-literature/CATALOGUE.md`, searched
> 2026-10-05), so nothing about it is stated as fact here. A sourced account,
> with page numbers, is waiting for the maintainer.*

**The question of this lesson:** *keep exactly the same atoms in exactly the
same proportion, but gather the uranium into lumps. Why should that help, and
by how much?*

---

## 1. A lump shields itself

**Answer.** Neutrons slow down in the graphite, not in the uranium (rung 2,
step 3: carbon takes 114 collisions to thermal, U-238 two thousand). So they
arrive at the lump from outside, already at resonance energies. At the energy
of a U-238 resonance peak, the uranium metal is so absorbing that a neutron
entering the lump is caught within a fraction of a millimetre of its surface.
The **inside of the lump never sees neutrons at that energy**: the surface
atoms have taken them all. Those interior atoms are wasted for resonance
capture.

That is **spatial self-shielding**, the same idea as the energy self-shielding
of rung 2's step 5, now in space. Fewer U-238 atoms are effectively
available to capture resonance neutrons, so more neutrons escape to thermal
energies: **$p$ rises**.

**Formula.** At one energy, for a sphere of radius $r$ and absorption cross
section $\Sigma$ in an isotropic flux, the probability that a neutron entering
the lump is absorbed is (exact, for a pure absorber)

$$P_\text{abs}(\tau) = 1 - \frac{1 - (1 + 2\tau) e^{-2\tau}}{2\tau^2}, \quad \tau = \Sigma r$$

If the same atoms were spread thinly, each entering neutron would meet them
over the sphere's **mean chord** $\bar\ell = 4r/3$ and be absorbed with
probability $\Sigma \bar\ell$. The ratio $P_\text{abs} / (\Sigma\bar\ell)$ is
how effective each atom in the lump still is: about 1 for a small or
transparent lump, and $\approx 1/(\Sigma\bar\ell)$, tiny, for a black one.

<div class="mcw" data-mc-widget="lump" data-tau="1"></div>

*Illustration (one energy, pure absorber, JavaScript's random numbers). Drag
$\Sigma r$ up, as at a resonance peak: absorptions crowd the surface and the
per-atom effectiveness collapses. Drag it down, as between resonances: the
lump is transparent and every atom counts.*

**A number to hold on to.** The lump is natural uranium metal, $N_U =
0.0482$ atoms/(b·cm) (19.05 g/cm³). At the 6.67 eV resonance peak U-238
captures with a cross section of thousands of barns, so the mean free path is
of order **10 µm**. Even a 0.1 mm lump is black at the peak, which matters
for the check in step 4.

<div class="predict">

**Predict.** If lumping is so good for $p$, why not one enormous lump? What
gets worse as the lump grows?

</div>

## 2. The price: thermal neutrons must reach the lump too

**Answer.** Once a neutron is thermal, we *want* it absorbed in uranium (U-235
fission), not in carbon. In a lump, the thermal neutrons diffusing around in
the graphite must find the lump, and the lump's own surface layers absorb
them before they reach its interior. The thermal flux inside the lump is
lower than in the graphite around it, so carbon gets a larger share of the
thermal absorptions: **$f$ falls**. The bigger the lump (at fixed average
composition, so the lumps are further apart), the larger this penalty.

So $k_\infty = \eta f p \varepsilon$ is a contest between $p$ rising and $f$
falling. $\eta$ is a property of the uranium (natural uranium's, as in rung
2) and barely moves; $\varepsilon$ rises a little, because a fast neutron born
in a lump meets more U-238 before it leaves it.

<div class="predict">

**Predict.** Plot $k_\infty$ against lump radius at a fixed carbon-to-uranium
ratio. What shape? Does the best lump get above 1?

</div>

## 3. One cell stands for the whole lattice

**Answer.** A real pile is a lattice of lumps in a graphite block. If the
lattice is large, every cell is like every other, so model **one cell** and
make whatever leaves it come back as if from the neighbour. The cell here is
the classic **Wigner–Seitz sphere**: the uranium sphere of radius $r$ at the
centre of a graphite sphere of radius $R$, with $R$ chosen so the cell holds
600 carbon atoms per uranium atom:

$$\frac{(1 - v) N_C}{v N_U} = 600, \quad v = \left(\frac{r}{R}\right)^3$$

which gives $R = 6.94\,r$ with $N_C = 0.0867$ and $N_U = 0.0482$
atoms/(b·cm) (`vv::ugraphite::cell_radius`).

**The boundary is white, not a mirror.** A neutron reaching $R$ is sent back
in with a random, cosine-law direction, as if a neighbour had emitted it. A
mirror sphere would be wrong: it keeps every neutron's distance of closest
approach to the centre for ever, so a neutron that misses the lump once misses
it on every bounce. In an earlier study here that error was a flat
+39–42 % that looked like physics (the warning in `lump_self_shielding_scan.rs`'s
`vv_gate` doc comment). The white boundary is the standard approximation for
a Wigner–Seitz cell; it is still an approximation of a real square or
hexagonal lattice.

The cell, drawn from the geometry the solver assembles (lump radius 2 cm,
cell radius 13.9 cm):

![Rung 3 cell, x-y slice through the centre](https://raw.githubusercontent.com/theodoreOnzGit/outram-park-backend/@@COMMIT@@/crates/outram-mc-libs/verification_and_validation/tutorial_rung3/geometry/ws_cell_r02.00cm_cell.png)

*(Every radius of the scan is drawn in
[`verification_and_validation/tutorial_rung3/geometry/`](https://github.com/theodoreOnzGit/outram-park-backend/tree/@@COMMIT@@/crates/outram-mc-libs/verification_and_validation/tutorial_rung3/geometry),
whole cell and lump, by `MODE=images` of the example.)*

**In the code.** The cell is built by the same constructor the pebble and
the lump scan use (a ball, two shells, a boundary condition):

> **On the [Code Review map](../../code-review/):** start at [`main`](../../code-review/#crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs::main) and follow its callees (▸) to [`fhr_pebble_geometry`](../../code-review/#crates/outram-mc-libs/src/pebble_beds/fhr_pebble.rs::fhr_pebble_geometry). Each card opens the function's source and shows its review stamp.

and a neutron reaching the outer surface takes the white branch of the
surface crossing (a port of OpenMC's `Surface::diffuse_reflect`, verified
against OpenMC in `verification_and_validation/white_boundary/`):

> **On the [Code Review map](../../code-review/):** start at [`transport_history_vr`](../../code-review/#crates/outram-mc-libs/src/physics/transport_csg.rs::transport_history_vr) and follow its callees (▸) to [`SurfaceKind::diffuse_reflect`](../../code-review/#crates/outram-mc-libs/src/geometry/crossing/mod.rs::SurfaceKind::diffuse_reflect). Each card opens the function's source and shows its review stamp.

Inside the cell, every collision takes exactly rung 2's path: the same
nuclide choice, the same S(α,β) / free-gas / at-rest fork, the same URR
tables. The only new thing is that a flight can now end on the lump's surface
and continue in the other material.

With the uranium in its own material, the library's own definition of "fuel"
(the material holding uranium) is the textbook one, so $f$ comes straight from
`run_keff_reactor_physics`, with no nuclide split:

> **On the [Code Review map](../../code-review/):** start at [`main`](../../code-review/#crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs::main) and follow its callees (▸) to [`run_keff_reactor_physics`](../../code-review/#crates/outram-mc-libs/src/physics/reactor_physics.rs::run_keff_reactor_physics). Each card opens the function's source and shows its review stamp.

<div class="predict">

**Predict.** Fill *both* regions of the cell with the same homogeneous
mixture, at 600 carbon atoms per uranium atom. What should $k_\infty$ be?

</div>

## 4. A check that can fail: the homogenised cell

**Answer.** If both regions hold the cell-average mixture, the cell is an
infinite homogeneous medium, the white boundary is then exact (the neutron
directions are isotropic everywhere), and $k_\infty$ must equal rung 2's
homogeneous result at the same ratio. That is not automatic: a flight lost or
counted twice where it crosses the inner surface or the white boundary would
show here. (Shrinking the lump towards zero cannot do this check affordably:
step 1's 10 µm mean free path means even a 0.1 mm lump is black at the
resonance peak.)

**Measured** (2026-10-05, the example's doc comment and
`verification_and_validation/tutorial_rung3/README.md`): the homogenised cell
gives $k_\infty = 0.77580 \pm 0.00208$; rung 2's homogeneous mixture at the
same ratio gave $0.77153 \pm 0.00243$. The difference, $+427 \pm 320$ pcm, is
1.3σ, inside the 3σ criterion written down before the run. **The check
passes**: the cell's surfaces and its white boundary lose and duplicate
nothing that this check could see.

## 5. $k_\infty$ against lump radius

**The prediction, written before the run** (2026-10-04, commit `43832d173`,
in the example's doc comment, not edited since): $k_\infty$ rises with $r$
from the homogeneous value, passes a maximum and falls. My expectation (from
memory of natural-uranium–graphite lattice physics, not a page-checked
source): a gain of tens of per cent, nearly all in $p$; **a maximum above 1,
of order 1.05, at a lump radius of order 1–3 cm**; $f$ falling a few per cent
over the range.

**What the code gives** (2026-10-05, 5000 neutrons × [20 + 50] generations
per cell, so each $k$ is good to about ±0.0025; recorded in the example's doc
comment and `verification_and_validation/tutorial_rung3/README.md`):

| lump $r$ (cm) | cell $R$ (cm) | $k_\infty$ | $\eta$ | $f$ | $p$ | $\varepsilon$ |
|---|---|---|---|---|---|---|
| homogenised (control) | 13.882 | 0.77580 ± 0.00208 | 1.33303 | 0.76483 | 0.74811 | 1.01241 |
| 0.1 | 0.694 | 0.93965 ± 0.00254 | 1.33288 | 0.76000 | 0.91854 | 1.01206 |
| 0.3 | 2.082 | **0.95924 ± 0.00211** | 1.33253 | 0.74922 | 0.94424 | 1.01542 |
| 1.0 | 6.941 | 0.93440 ± 0.00288 | 1.33130 | 0.70584 | 0.96285 | 1.02887 |
| 2.0 | 13.882 | 0.85390 ± 0.00239 | 1.32966 | 0.62863 | 0.96981 | 1.05332 |
| 3.0 | 20.823 | 0.76770 ± 0.00295 | 1.32862 | 0.54765 | 0.97087 | 1.08605 |
| 4.0 | 27.764 | 0.68685 ± 0.00252 | 1.32781 | 0.47341 | 0.96517 | 1.13058 |
| 6.0 | 41.646 | 0.55402 ± 0.00195 | 1.32608 | 0.35050 | 0.93550 | 1.27094 |

- **Lumping helps enormously, and at once.** Even 1 mm lumps lift
  $k_\infty$ from 0.776 to 0.940, almost all through $p$ (0.748 → 0.919): at
  a resonance peak the lump is black whatever its size, so its interior is
  shielded from the start.
- **Then $f$ takes over.** Bigger lumps, at the same average composition, sit
  further apart in more graphite, and the thermal neutrons are absorbed in
  the graphite before they find one: $f$ falls from 0.76 to 0.35 across the
  scan. The peak is **0.959 ± 0.002 at $r$ = 0.3 cm**.
- **My prediction was wrong twice.** I expected a peak above 1 (about 1.05)
  at a radius of 1–3 cm. Both are refuted: the best cell is 19σ short of 1,
  at a few millimetres. The rise-then-fall shape and "the gain is in $p$"
  held.
- Not predicted: in a thick lump the fast neutrons meet a lot of U-238
  before they leave, and $\varepsilon$ climbs to 1.27 at 6 cm.

**Why, and the next question.** At 600 carbon atoms per uranium atom, $f$
was already low before lumping (0.765). So try less carbon. That second
question was written down, with its prediction ($k_\infty > 1$ at some radius
around 1–2 cm, at $N_C/N_U = 200$), *after* seeing this scan and *before*
running it (commit `bd05a299c`, `verification_and_validation/tutorial_rung3/README.md`).

**The answer** (2026-10-05, same settings, recorded beside the first scan):

| lump $r$ (cm) | cell $R$ (cm) | $k_\infty$ | $\eta$ | $f$ | $p$ | $\varepsilon$ |
|---|---|---|---|---|---|---|
| 0.3 | 1.447 | 1.04795 ± 0.00257 | 1.32800 | 0.89991 | 0.84795 | 1.03228 |
| 1.0 | 4.822 | **1.08986 ± 0.00233** | 1.32738 | 0.88001 | 0.89753 | 1.03916 |
| 2.0 | 9.644 | 1.08002 ± 0.00220 | 1.32653 | 0.84273 | 0.91759 | 1.05065 |
| 3.0 | 14.466 | 1.04409 ± 0.00239 | 1.32604 | 0.79521 | 0.92777 | 1.06298 |

**Above 1.** With a third of the carbon, the best lump ($r$ = 1 cm, cell
radius 4.8 cm) gives $k_\infty = 1.090 \pm 0.002$: an infinite lattice of these
cells would sustain a chain reaction, from natural uranium. $f$ stays at
0.80–0.90 here, while $p$ still climbs with the lump. The homogeneous mixture
at this ratio managed 0.726 (rung 2): lumping is worth about **+0.36** in
$k_\infty$. That is why the pile was built from lumps.

(A real pile is finite and leaks, and its graphite and uranium were not this
pure; an infinite-lattice $k_\infty$ above 1 is necessary, not sufficient.
The *Create* exercise below asks how much the spherical cell itself is worth.)

**Verification, not validation.** No experiment was done on this cell; the
checks are the homogeneous limit and the algebra of the factors. An OpenMC
code-to-code comparison on a few radii is prepared (the deck is committed:
`verification_and_validation/tutorial_rung3/openmc_inputs/lumped_openmc.py`)
but has **not been run** for these results: OpenMC was not available on the
machine that ran them. It is pending the maintainer.

**Run it yourself:**

```text
cargo run --release -p outram-mc-libs --features endf-pebble-cases --example lumped_ugraphite_kinf
```

`RADII=1,2,3` picks the lump radii (cm), `CU` the cell-average ratio,
`CONTROL=0` skips the homogenised cell; `PARTICLES`, `INACTIVE`, `ACTIVE`,
`THREADS` set the size. `MODE=images` draws the cells.

**Modify.** Run the scan at `CU=300` and at `CU=1000`. Predict first: does
the best radius move, and which way?

**Create.** The pile's lumps sat in a cubic lattice, not in spheres. Build a
cubic cell (a uranium sphere in a reflective graphite cube, `homogeneous_cube`
plus a sphere) at the same ratio and radius, and compare $k_\infty$ with the
white Wigner–Seitz sphere. How big is the difference the approximation makes?

## Next rung

Lumps work. Now swap graphite for **water**, and the lumps for **rods** in a
regular lattice: the light-water reactor, [rung 4](lct008.md).

## The whole call tree

Everything `lumped_ugraphite_kinf.rs`'s `main` reaches inside the workspace,
on the [Code Review map](../../code-review/): unfold its callees (▸) level by level.

> **On the [Code Review map](../../code-review/):** start at [`main`](../../code-review/#crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs::main). Each card opens the function's source and shows its review stamp.

In the demo, the worker runs the same transport, one neutron per call,
through
[`ugraphite::sim::Chain::run_next`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/dhoby-ghaut/examples/monte_carlo_web/ugraphite/sim.rs#@@L:crates/dhoby-ghaut/examples/monte_carlo_web/ugraphite/sim.rs:fn=run_next@@)
on the cell of
[`lumped::model::build_geometry`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/dhoby-ghaut/examples/monte_carlo_web/lumped/model.rs#@@L:crates/dhoby-ghaut/examples/monte_carlo_web/lumped/model.rs:fn=build_geometry@@)
*(filled by hand: page and worker talk by messages, not calls)*.

---

**Deliberate liberties.** The lump is natural uranium **metal** at 19.05 g/cm³
(a handbook density, not page-checked); the pile's real fuel (metal and
oxide, in shapes that were not spheres) is not modelled. The cell is a
Wigner–Seitz sphere with a white boundary, an approximation of a cubic
lattice whose size has not been measured here (the "Create" exercise). Natural
uranium is the IUPAC composition, recalled and not page-checked. Everything is
at 296 K. The demo's Watch mode processes the data at tolerance 0.01 (NJOY's is
0.001).

**Literature.** ENDF/B-VIII.0 [(Brown & others, 2018)](#ref-brown2018endf8) for every cross section and the graphite thermal scattering law. The
sphere's absorption probability in step 1 is the standard first-flight result
for a sphere in an isotropic flux (checked here only by sampling it, as the
widget does); no page-cited textbook is in the corpus yet. For multi-region
cells, the crate's `physics::collision_probability` computes exact
first-flight collision probabilities (the deterministic reference of
`lump_self_shielding_scan.rs`).

**Doesn't tally?** If anything here disagrees with the code it links to, the
page is wrong:
[report it](https://github.com/theodoreOnzGit/outram-park-backend/issues/new?title=Lumping%20lesson%20doesn%27t%20tally%3A%20&labels=bug).
This page changes whenever `develop` does; it was built from
`@@COMMIT_SHORT@@` on @@BUILD_DATE@@. Tracking issue
[#525](https://github.com/theodoreOnzGit/outram-park-backend/issues/525).

<!-- references:begin -->
## References

<p class="csl-entry" id="ref-brown2018endf8" style="padding-left: 2em; text-indent: -2em;">Brown, D. A. &#38; others. (2018). ENDF/B-VIII.0: The 8th Major Release of the Nuclear Reaction Data Library with CIELO-project Cross Sections, New Standards and Thermal Scattering Data. <i>Nuclear Data Sheets</i>, <i>148</i>, 1–142. https://doi.org/10.1016/j.nds.2018.02.001</p>

<!-- references:end -->
