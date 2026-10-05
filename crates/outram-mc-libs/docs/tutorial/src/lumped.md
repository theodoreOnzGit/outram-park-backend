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
Its answer (the natural-uranium sweep of step 7) is being measured: *Running on 2026-10-05; the measured result is recorded here in the next update of this page.*

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

**The code walk.** The cell is built by the same constructor the pebble and
the lump scan use (a ball, two shells, a boundary condition):

<div class="codewalk">

<!-- code-walk: from=crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs::main to=crates/outram-mc-libs/src/pebble_beds/fhr_pebble.rs::fhr_pebble_geometry depth=4 -->
<!-- /code-walk -->

</div>

and a neutron reaching the outer surface takes the white branch of the
surface crossing (a port of OpenMC's `Surface::diffuse_reflect`, verified
against OpenMC in `verification_and_validation/white_boundary/`):

<div class="codewalk">

<!-- code-walk: from=crates/outram-mc-libs/src/physics/transport_csg.rs::transport_history_vr to=crates/outram-mc-libs/src/geometry/crossing/mod.rs::SurfaceKind::diffuse_reflect depth=4 -->
<!-- /code-walk -->

</div>

Inside the cell, every collision takes exactly rung 2's path: the same
nuclide choice, the same S(α,β) / free-gas / at-rest fork, the same URR
tables. The only new thing is that a flight can now end on the lump's surface
and continue in the other material.

With the uranium in its own material, the library's own definition of "fuel"
(the material holding uranium) is the textbook one, so $f$ comes straight from
`run_keff_reactor_physics`, with no nuclide split:

<div class="codewalk">

<!-- code-walk: from=crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs::main to=crates/outram-mc-libs/src/physics/reactor_physics.rs::run_keff_reactor_physics depth=6 -->
<!-- /code-walk -->

</div>

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

*Running on 2026-10-05; the measured result is recorded here in the next update of this page.*

## 5. $k_\infty$ against lump radius

**The prediction, written before the run** (2026-10-04, commit `43832d173`,
in the example's doc comment, not edited since): $k_\infty$ rises with $r$
from the homogeneous value, passes a maximum and falls. My expectation (from
memory of natural-uranium–graphite lattice physics, not a page-checked
source): a gain of tens of per cent, nearly all in $p$; **a maximum above 1,
of order 1.05, at a lump radius of order 1–3 cm**; $f$ falling a few per cent
over the range.

*Running on 2026-10-05; the measured result is recorded here in the next update of this page.*

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
three calls deep. Generated by `kovan-cli code-walk`.

<div class="codewalk">

<!-- code-walk: from=crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs::main depth=3 -->
<!-- /code-walk -->

</div>

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

**Literature.** ENDF/B-VIII.0 (Brown et al., *Nuclear Data Sheets* 148,
2018) for every cross section and the graphite thermal scattering law. The
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
