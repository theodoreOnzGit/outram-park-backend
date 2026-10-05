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

**The code walk.** The cell is built by the same constructor the pebble and
the lump scan use (a ball, two shells, a boundary condition):

<div class="codewalk">

<!-- code-walk: from=crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs::main to=crates/outram-mc-libs/src/pebble_beds/fhr_pebble.rs::fhr_pebble_geometry depth=4 -->
<!-- generated by `kovan-cli code-walk-check --update`; edit the comment above, not this -->

Call chain from `lumped_ugraphite_kinf.rs::main` to `fhr_pebble.rs::fhr_pebble_geometry`: 2 hops, 1 shortest chain. Each step shows its code; the name links to it on GitHub.

**1.** [`lumped_ugraphite_kinf.rs::main`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L214)

<!-- snippet-check: crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs:214 fn main -->
<!-- snippet-check: crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs:258 ws_cell -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs:214:214}}
    // …
{{#include ../../../../../crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs:256:259}}
    // … (the rest of the function: follow the link above)
```

**2.** → [`lumped_ugraphite_kinf.rs::ws_cell`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L200) · called at [L258](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L258) — The Wigner-Seitz cell: lump (material 0) to `r`, graphite (material 1) to `big_r`, white outer boundary.

<!-- snippet-check: crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs:200 fn ws_cell -->
<!-- snippet-check: crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs:201 fhr_pebble_geometry -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs:200:202}}
    // … (the rest of the function: follow the link above)
```

**3.** → [`fhr_pebble.rs::fhr_pebble_geometry`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/fhr_pebble.rs#L427) · called at [L201](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L201) — Concentric-shell geometry of one FHR pebble, for the CSG drivers.

<!-- snippet-check: crates/outram-mc-libs/src/pebble_beds/fhr_pebble.rs:427 fn fhr_pebble_geometry -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/pebble_beds/fhr_pebble.rs:427:466}}
    // … (the rest of the function: follow the link above)
```

Unresolved calls inside the functions on this chain:

- in [`lumped_ugraphite_kinf.rs::main`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L214):
  - UNRESOLVED(closure): `cell_r` at [L258](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L258) (→ [`crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs:221`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L221)) — call through a closure or fn-typed binding `cell_r`
  - UNRESOLVED(closure): `cell_r` at [L277](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L277) (→ [`crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs:221`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L221)) — call through a closure or fn-typed binding `cell_r`
  - UNRESOLVED(closure): `cell_r` at [L303](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L303) (→ [`crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs:221`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L221)) — call through a closure or fn-typed binding `cell_r`
<!-- /code-walk -->

</div>

and a neutron reaching the outer surface takes the white branch of the
surface crossing (a port of OpenMC's `Surface::diffuse_reflect`, verified
against OpenMC in `verification_and_validation/white_boundary/`):

<div class="codewalk">

<!-- code-walk: from=crates/outram-mc-libs/src/physics/transport_csg.rs::transport_history_vr to=crates/outram-mc-libs/src/geometry/crossing/mod.rs::SurfaceKind::diffuse_reflect depth=4 -->
<!-- generated by `kovan-cli code-walk-check --update`; edit the comment above, not this -->

Call chain from `transport_csg.rs::transport_history_vr` to `mod.rs::SurfaceKind::diffuse_reflect`: 3 hops, 1 shortest chain. Each step shows its code; the name links to it on GitHub.

**1.** [`transport_csg.rs::transport_history_vr`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1398) — One history of the CSG k-eigenvalue kernel, with an explicit variance-reduction configuration.

<!-- snippet-check: crates/outram-mc-libs/src/physics/transport_csg.rs:1398 fn transport_history_vr -->
<!-- snippet-check: crates/outram-mc-libs/src/physics/transport_csg.rs:2467 cross_surface_in_frame -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/physics/transport_csg.rs:1398:1405}}
    // …
{{#include ../../../../../crates/outram-mc-libs/src/physics/transport_csg.rs:2465:2468}}
    // … (the rest of the function: follow the link above)
```

**2.** → [`mod.rs::Geometry::cross_surface_in_frame`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/geometry/crossing/mod.rs#L563) · called at [L2467](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2467)

<!-- snippet-check: crates/outram-mc-libs/src/geometry/crossing/mod.rs:563 fn cross_surface_in_frame -->
<!-- snippet-check: crates/outram-mc-libs/src/geometry/crossing/mod.rs:574 cross_surface -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/geometry/crossing/mod.rs:563:575}}
    // … (the rest of the function: follow the link above)
```

**3.** → [`mod.rs::Geometry::cross_surface`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/geometry/crossing/mod.rs#L471) · called at [L574](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/geometry/crossing/mod.rs#L574)

<!-- snippet-check: crates/outram-mc-libs/src/geometry/crossing/mod.rs:471 fn cross_surface -->
<!-- snippet-check: crates/outram-mc-libs/src/geometry/crossing/mod.rs:502 diffuse_reflect -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/geometry/crossing/mod.rs:471:477}}
    // …
{{#include ../../../../../crates/outram-mc-libs/src/geometry/crossing/mod.rs:500:503}}
    // … (the rest of the function: follow the link above)
```

**4.** → [`mod.rs::SurfaceKind::diffuse_reflect`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/geometry/crossing/mod.rs#L133) · called at [L502](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/geometry/crossing/mod.rs#L502)

<!-- snippet-check: crates/outram-mc-libs/src/geometry/crossing/mod.rs:133 fn diffuse_reflect -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/geometry/crossing/mod.rs:133:153}}
```

Unresolved calls inside the functions on this chain:

- in [`transport_csg.rs::transport_history_vr`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1398):
  - UNRESOLVED(no-definition): `Some` at [L1643](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1643) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `as_deref_mut` at [L1643](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1643) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `record` at [L1644](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1644) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `Some` at [L1668](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1668) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `as_ref` at [L1668](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1668) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `Some` at [L1669](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1669) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `look_up` at [L1669](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1669) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `apply_window` at [L1670](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1670) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `future_seed` at [L1689](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1689) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `push` at [L1690](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1690) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `Some` at [L1698](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1698) — rust-analyzer returned no definition
  - UNRESOLVED(macro): `track!` at [L1730](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1730) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1641`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1641)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L1739](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1739) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1641`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1641)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2075](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2075) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1641`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1641)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2133](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2133) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1641`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1641)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2136](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2136) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1641`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1641)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2421](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2421) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1641`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1641)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `weight_window_checkpoint!` at [L2426](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2426) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1666`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1666)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2427](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2427) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1641`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1641)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2473](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2473) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1641`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1641)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2482](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2482) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1641`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1641)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `weight_window_checkpoint!` at [L2498](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2498) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1666`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1666)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2499](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2499) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1641`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1641)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2511](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2511) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1641`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1641)) — workspace macro; its expansion is not followed
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
<!-- generated by `kovan-cli code-walk-check --update`; edit the comment above, not this -->

Call chain from `lumped_ugraphite_kinf.rs::main` to `reactor_physics.rs::run_keff_reactor_physics`: 2 hops, 1 shortest chain. Each step shows its code; the name links to it on GitHub.

**1.** [`lumped_ugraphite_kinf.rs::main`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L214)

<!-- snippet-check: crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs:214 fn main -->
<!-- snippet-check: crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs:260 run_case -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs:214:214}}
    // …
{{#include ../../../../../crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs:258:261}}
    // … (the rest of the function: follow the link above)
```

**2.** → [`ugraphite_common.rs::run_case`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L170) · called at [L260](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L260) — Run the power iteration with six-factor capture on `geom` and assemble the factors with the requested fuel definition.

<!-- snippet-check: crates/outram-mc-libs/examples/common/ugraphite_common.rs:170 fn run_case -->
<!-- snippet-check: crates/outram-mc-libs/examples/common/ugraphite_common.rs:187 run_keff_reactor_physics -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/examples/common/ugraphite_common.rs:170:188}}
    // … (the rest of the function: follow the link above)
```

**3.** → [`reactor_physics.rs::run_keff_reactor_physics`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L575) · called at [L187](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L187) — Run a k-eigenvalue power iteration over `geom` and capture the six-factor decomposition and the lethargy-normalised flux spectrum from one combined track-length tally plus explicit leakage accounting.

<!-- snippet-check: crates/outram-mc-libs/src/physics/reactor_physics.rs:575 fn run_keff_reactor_physics -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/physics/reactor_physics.rs:575:614}}
    // … (the rest of the function: follow the link above)
```

Unresolved calls inside the functions on this chain:

- in [`lumped_ugraphite_kinf.rs::main`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L214):
  - UNRESOLVED(closure): `cell_r` at [L258](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L258) (→ [`crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs:221`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L221)) — call through a closure or fn-typed binding `cell_r`
  - UNRESOLVED(closure): `cell_r` at [L277](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L277) (→ [`crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs:221`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L221)) — call through a closure or fn-typed binding `cell_r`
  - UNRESOLVED(closure): `cell_r` at [L303](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L303) (→ [`crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs:221`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L221)) — call through a closure or fn-typed binding `cell_r`
- in [`ugraphite_common.rs::run_case`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L170):
  - UNRESOLVED(other): `clone` at [L192](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L192) (→ [`crates/outram-mc-libs/src/physics/reactor_physics.rs:229`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L229)) — resolves to `#[derive(Debug, Clone)]`, not a function body
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
three calls deep. Generated by `kovan-cli code-walk`.

<div class="codewalk">

<!-- code-walk: from=crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs::main depth=3 -->
<!-- generated by `kovan-cli code-walk-check --update`; edit the comment above, not this -->

Everything `crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs::main` reaches in the workspace, to 3 hops: 65 functions, 14 unresolved calls. A function is expanded once; later calls to it say *(expanded elsewhere in this walk)*.

- [`lumped_ugraphite_kinf.rs::main`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L214) `fn main()`
  - [`ugraphite_common.rs::env_or`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L75) `pub fn env_or<T: std::str::FromStr>(k: &str, d: T) -> T` · called at [L215](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L215)
  - [`ugraphite.rs::uranium_volume_fraction`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/vv/ugraphite.rs#L210) `pub fn uranium_volume_fraction(c_per_u: f64) -> f64` — Uranium volume fraction of a cell whose uranium-metal lump and pure graphite give a cell-average `c_per_u`: `(1 - v) N_C / (v N_U) = c_per_u`. · called at [L216](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L216)
    - `ugraphite.rs::uranium_metal` *(expanded elsewhere in this walk)* · called at [L211](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/vv/ugraphite.rs#L211)
    - [`ugraphite.rs::graphite_density`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/vv/ugraphite.rs#L174) `pub fn graphite_density() -> f64` — Graphite at 1.73 g/cm3 (`RHO_GRAPHITE`) \[atoms/b-cm\]. · called at [L212](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/vv/ugraphite.rs#L212)
  - [`ugraphite.rs::cell_radius`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/vv/ugraphite.rs#L217) `pub fn cell_radius(r_lump: f64, c_per_u: f64) -> f64` — Wigner-Seitz cell radius for a lump of radius `r_lump` at cell-average `c_per_u` \[cm\]. · called at [L221](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L221)
    - `ugraphite.rs::uranium_volume_fraction` *(expanded elsewhere in this walk)* · called at [L218](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/vv/ugraphite.rs#L218)
  - [`lumped_ugraphite_kinf.rs::draw`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L318) `fn draw(radii: &[f64], cu: f64, cell_r: impl Fn(f64) -> f64)` — Draw what the solver sees: an x-y slice through the centre of each cell, coloured by material, from the assembled geometry. · called at [L224](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L224)
    - [`colour.rs::Rgb::new`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-blender/src/csg/plot/colour.rs#L40) `pub const fn new(r: u8, g: u8, b: u8) -> Self` — A colour from its three channels. · called at [L323](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L323)
    - `lumped_ugraphite_kinf.rs::draw` *(expanded elsewhere in this walk)* · called at [L327](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L327)
    - `lumped_ugraphite_kinf.rs::ws_cell` *(expanded elsewhere in this walk)* · called at [L328](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L328)
    - [`slice.rs::SlicePlot::new`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-blender/src/csg/plot/slice.rs#L162) `pub fn new(basis: PlotBasis, origin: Position, width: [f64; 2], pixels: [usize; 2]) -> Self` — A slice with upstream's defaults: leaf level, no overlaps, no mesh lines. · called at [L331](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L331)
    - [`position.rs::Position::new`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-blender/src/csg/position.rs#L64) `pub fn new(x: f64, y: f64, z: f64) -> Self` · called at [L333](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L333)
    - [`mod.rs::render_material_slice`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-blender/src/csg/plot/mod.rs#L122) `pub fn render_material_slice(geom: &Geometry, plot: &SlicePlot, palette: &[(Rgb, &str)], title: &str) -> (ImageData, ImageData)` — **Draw a slice of an assembled geometry by material, with a legend and dimensioned axes** — the geometry-drawing rule's minimum, in one call. · called at [L337](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L337)
      - [`mod.rs::material_count`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-blender/src/csg/plot/mod.rs#L101) `pub fn material_count(geom: &Geometry) -> usize` — Highest material index any cell fills with, plus one — the smallest material table a `ColourScheme` for this geometry can use. · called at [L128](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-blender/src/csg/plot/mod.rs#L128) · *(calls below the depth limit not shown)*
      - [`colour.rs::ColourScheme::new`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-blender/src/csg/plot/colour.rs#L150) `pub fn new(colour_by: PlotColourBy, n_domains: usize, seed: &mut u64) -> Self` — Default colours for `n_domains` cells or materials, drawn from `seed`. · called at [L130](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-blender/src/csg/plot/mod.rs#L130) · *(calls below the depth limit not shown)*
      - `colour.rs::Rgb::new` *(expanded elsewhere in this walk)* · called at [L131](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-blender/src/csg/plot/mod.rs#L131)
      - [`colour.rs::ColourScheme::with_background`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-blender/src/csg/plot/colour.rs#L186) `pub fn with_background(mut self, colour: Rgb) -> Self` — Background colour. · called at [L131](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-blender/src/csg/plot/mod.rs#L131) · *(calls below the depth limit not shown)*
      - [`colour.rs::ColourScheme::with_colour`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-blender/src/csg/plot/colour.rs#L164) `pub fn with_colour(mut self, index: usize, colour: Rgb) -> Self` — Set one cell's or material's colour, by index. · called at [L133](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-blender/src/csg/plot/mod.rs#L133) · *(calls below the depth limit not shown)*
      - [`slice.rs::SlicePlot::id_map`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-blender/src/csg/plot/slice.rs#L235) `pub fn id_map(&self, geom: &Geometry) -> IdMap` — Locate every pixel. · called at [L135](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-blender/src/csg/plot/mod.rs#L135) · *(calls below the depth limit not shown)*
      - [`slice.rs::SlicePlot::colour_id_map`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-blender/src/csg/plot/slice.rs#L283) `pub fn colour_id_map(&self, ids: &IdMap, scheme: &ColourScheme) -> ImageData` — Colour an already-computed `IdMap` — the colouring half of `create_image`, so one geometry pass can be drawn by cell and by material without locating every pixel twice. · called at [L136](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-blender/src/csg/plot/mod.rs#L136) · *(calls below the depth limit not shown)*
      - [`annotate.rs::LegendEntry::new`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-blender/src/csg/plot/annotate.rs#L46) `pub fn new(colour: Rgb, label: impl Into<String>) -> Self` — A legend row. · called at [L159](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-blender/src/csg/plot/mod.rs#L159) · *(calls below the depth limit not shown)*
      - [`annotate.rs::annotate_slice`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-blender/src/csg/plot/annotate.rs#L250) `pub fn annotate_slice(image: &ImageData, plot: &SlicePlot, title: &str, legend: &[LegendEntry]) -> ImageData` — Frame a slice image with a title, a legend, and tick marks labelled in cm along the bottom (horizontal axis) and left (vertical axis) edges. · called at [L168](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-blender/src/csg/plot/mod.rs#L168) · *(calls below the depth limit not shown)*
    - [`image.rs::ImageData::write_png`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-blender/src/csg/plot/image.rs#L95) `pub fn write_png(&self, path: impl AsRef<Path>) -> io::Result<()>` — Write a PNG file. · called at [L346](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L346)
      - [`image.rs::ImageData::to_png_bytes`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-blender/src/csg/plot/image.rs#L87) `pub fn to_png_bytes(&self) -> Vec<u8>` — Encode as PNG: 8-bit RGB, non-interlaced — the same `IHDR` upstream's `output_png` asks libpng for (`src/plot.cpp:909-910`). · called at [L96](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-blender/src/csg/plot/image.rs#L96) · *(calls below the depth limit not shown)*
  - [`ugraphite_common.rs::load_nuclides`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L89) `pub fn load_nuclides() -> Vec<Nuclide>` — U-234, U-235, U-238, C-12, C-13 from ENDF/B-VIII.0 at `TEMP_K` (RECONR + BROADR, tolerance 0.001; URR and DBRC by the constructor's defaults), with crystalline-graphite S(alpha,beta) (MAT 30, 296 K) on both carbons. · called at [L231](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L231)
    - [`reference_data.rs::reference_endf`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/reference_data.rs#L91) `pub fn reference_endf(file: &str) -> Option<PathBuf>` — Absolute path of reference tape `file` (e.g. · called at [L96](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L96)
      - [`reference_data.rs::reference_endf_dir`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/reference_data.rs#L52) `pub fn reference_endf_dir() -> PathBuf` — The directory reference tapes are read from, whether or not it exists. · called at [L94](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/reference_data.rs#L94) · *(calls below the depth limit not shown)*
    - [`nuclide.rs::Nuclide::from_endf_file`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L1642) `pub fn from_endf_file(path: &std::path::Path, name: &str, temp_k: f64, tolerance: f64) -> Result<Self, NjoyError>` — Build a nuclide from an ENDF file **on disk** — the ordinary case. · called at [L98](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L98)
      - [`tape.rs::Tape::read_file`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/endf/tape.rs#L98) `pub fn read_file(path: &std::path::Path) -> Result<Self, NjoyError>` — Parse an ENDF ASCII tape from a file on disk. · called at [L1648](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L1648) · *(calls below the depth limit not shown)*
      - [`tape.rs::Tape::materials`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/endf/tape.rs#L225) `pub fn materials(&self) -> Vec<i32>` — Every ENDF material number on this tape, ascending and deduplicated. · called at [L1652](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L1652) · *(calls below the depth limit not shown)*
      - [`nuclide.rs::Nuclide::from_tape`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L2360) `pub fn from_tape(tape: &njoy_outram_park_fork::endf::tape::Tape, mat: i32, name: &str, temp_k: f64, tolerance: f64) -> Result<Self, NjoyError>` — Build a nuclide from an ENDF tape **already in hand** — no network, no feature gate. · called at [L1655](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L1655) · *(calls below the depth limit not shown)*
    - [`thermal.rs::ThermalScattering::from_endf_file`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/thermal.rs#L639) `pub fn from_endf_file(path: &str, mat: i32, temperature_k: f64, name: &str) -> Result<Self, NjoyError>` — Build the pre-tabulated bound-atom thermal treatment — inelastic **and** elastic — from an ENDF `tsl-*` thermal evaluation file. · called at [L104](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L104)
      - [`tape.rs::Tape::read`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/endf/tape.rs#L113) `pub fn read<R: Read>(reader: R) -> Result<Self, NjoyError>` — Parse an ENDF ASCII tape from any `Read` source. · called at [L647](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/thermal.rs#L647) · *(calls below the depth limit not shown)*
      - [`thermal.rs::ThermalScattering::from_tape`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/thermal.rs#L818) `pub fn from_tape(tape: &njoy_outram_park_fork::endf::tape::Tape, mat: i32, temperature_k: f64, name: &str) -> Result<Self, NjoyError>` · called at [L648](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/thermal.rs#L648) · *(calls below the depth limit not shown)*
    - UNRESOLVED(closure): `load` at [L109](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L109) (→ [`crates/outram-mc-libs/examples/common/ugraphite_common.rs:95`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L95)) — call through a closure or fn-typed binding `load`
    - [`nuclide.rs::Nuclide::with_thermal_scattering`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L605) `pub fn with_thermal_scattering(mut self, thermal: ThermalScattering) -> Self` — Attach a bound-atom S(α,β) `ThermalScattering` treatment to this nuclide (builder style, consumes and returns `self`). · called at [L111](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L111)
    - UNRESOLVED(other): `clone` at [L111](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L111) (→ [`crates/outram-mc-libs/src/material/thermal.rs:542`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/thermal.rs#L542)) — resolves to `#[derive(Debug, Clone)]`, not a function body
  - [`ugraphite.rs::uranium_metal`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/vv/ugraphite.rs#L191) `pub fn uranium_metal() -> Mix` — Natural uranium metal at `RHO_U_METAL` \[atoms/b-cm\]. · called at [L233](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L233)
  - [`ugraphite.rs::graphite`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/vv/ugraphite.rs#L201) `pub fn graphite() -> Mix` — Pure graphite at 1.73 g/cm3, as a `Mix`. · called at [L234](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L234)
    - `ugraphite.rs::graphite_density` *(expanded elsewhere in this walk)* · called at [L204](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/vv/ugraphite.rs#L204)
  - [`ugraphite.rs::Mix::scaled`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/vv/ugraphite.rs#L116) `pub fn scaled(&self, factor: f64) -> Mix` — The same atoms scaled by `factor` (used to homogenise a cell). · called at [L254](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L254)
  - [`lumped_ugraphite_kinf.rs::ws_cell`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L200) `fn ws_cell(r: f64, big_r: f64) -> Geometry` — The Wigner-Seitz cell: lump (material 0) to `r`, graphite (material 1) to `big_r`, white outer boundary. · called at [L258](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L258)
    - [`fhr_pebble.rs::fhr_pebble_geometry`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/fhr_pebble.rs#L427) `pub fn fhr_pebble_geometry(r_inner: f64, r_fuel_outer: f64, r_shell_outer: f64, r_root: f64, fuel_mat: usize, graphite_mat: usize, coolant_mat: usize, root_bc: BoundaryType, temperature: f64) -> Geometry` — Concentric-shell geometry of one FHR pebble, for the CSG drivers. · called at [L201](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L201)
      - [`fhr_pebble.rs::sphere`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/fhr_pebble.rs#L511) `fn sphere(r: f64, bc: BoundaryType) -> SurfaceKind` · called at [L450](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/fhr_pebble.rs#L450) · *(calls below the depth limit not shown)*
      - [`cell.rs::Cell::material`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-blender/src/csg/cell.rs#L231) `pub fn material(id: i32, region: Vec<RegionToken>, material_idx: usize, temperature: f64) -> Self` — Build a material cell with no translation — the common leaf case. · called at [L465](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/fhr_pebble.rs#L465) · *(calls below the depth limit not shown)*
      - [`fhr_pebble.rs::inside`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/fhr_pebble.rs#L521) `fn inside(surface_idx: usize) -> RegionToken` · called at [L467](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/fhr_pebble.rs#L467) · *(calls below the depth limit not shown)*
      - [`fhr_pebble.rs::outside`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/fhr_pebble.rs#L528) `fn outside(surface_idx: usize) -> RegionToken` · called at [L477](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/fhr_pebble.rs#L477) · *(calls below the depth limit not shown)*
  - UNRESOLVED(closure): `cell_r` at [L258](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L258) (→ [`crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs:221`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L221)) — call through a closure or fn-typed binding `cell_r`
  - [`ugraphite.rs::Mix::material`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/vv/ugraphite.rs#L97) `pub fn material(&self, id: i32, name: &str) -> Material` — The material, with nuclide indices in `TAPES` order and zero densities dropped, at `TEMPERATURE_K`. · called at [L259](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L259)
    - [`ugraphite.rs::Mix::densities`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/vv/ugraphite.rs#L85) `pub fn densities(&self) -> [f64; 5]` — Atom densities in `TAPES` order: U-234, U-235, U-238, C-12, C-13. · called at [L103](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/vv/ugraphite.rs#L103)
  - [`ugraphite_common.rs::run_case`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L170) `pub fn run_case(geom: &Geometry, mats: &[Material], nuclides: &[Nuclide], seed: u64, source_half_cm: f64, split: FuelSplit) -> CaseResult` — Run the power iteration with six-factor capture on `geom` and assemble the factors with the requested fuel definition. · called at [L260](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L260)
    - [`ugraphite_common.rs::physics_config`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L147) `pub fn physics_config(seed: u64, source: SourceBox) -> ReactorPhysicsConfig` — The run settings every case uses, from the environment: `PARTICLES` (20000), `INACTIVE` (20), `ACTIVE` (100), `THREADS` (3). · called at [L179](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L179)
      - `ugraphite_common.rs::env_or` *(expanded elsewhere in this walk)* · called at [L148](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L148)
      - [`keff.rs::KeffSettings::default`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/keff.rs#L200) `fn default() -> Self` — A modest run (2000 histories × [30 inactive + 70 active]) with the U-235 thermal Watt spectrum, on the single-thread deterministic reference backend. · called at [L157](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L157) · *(calls below the depth limit not shown)*
    - `position.rs::Position::new` *(expanded elsewhere in this walk)* · called at [L182](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L182)
    - [`reactor_physics.rs::run_keff_reactor_physics`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L575) `pub fn run_keff_reactor_physics(geom: &Geometry, materials: &[Material], nuclides: &[Nuclide], config: &ReactorPhysicsConfig) -> Result<ReactorPhysicsReport, ReactorPhysicsError>` — Run a k-eigenvalue power iteration over `geom` and capture the six-factor decomposition and the lethargy-normalised flux spectrum from one combined track-length tally plus explicit leakage accounting. · called at [L187](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L187)
      - [`reactor_physics.rs::fine_energy_grid`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L437) `fn fine_energy_grid(cfg: &ReactorPhysicsConfig) -> Result<Vec<f64>, ReactorPhysicsError>` — The fine grid with the two group boundaries forced onto edges. · called at [L598](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L598) · *(calls below the depth limit not shown)*
      - UNRESOLVED(other): `default` at [L617](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L617) (→ [`crates/outram-mc-libs/src/tally/tally.rs:73`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/tally/tally.rs#L73)) — resolves to `#[derive(Debug, Default, Clone, PartialEq)]`, not a function body
      - UNRESOLVED(other): `default` at [L619](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L619) (→ [`crates/outram-mc-libs/src/tally/tally.rs:73`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/tally/tally.rs#L73)) — resolves to `#[derive(Debug, Default, Clone, PartialEq)]`, not a function body
      - [`transport_csg.rs::run_keff_csg_reactor_physics`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L506) `pub fn run_keff_csg_reactor_physics(geom: &Geometry, materials: &[Material], nuclides: &[Nuclide], source_box: SourceBox, settings: &KeffSettings, tally: &mut Tally, leak_edges: &[f64], leak_bins: &mut Vec<TallyBin>) -> KeffResult` — Like `run_keff_csg`, but also accumulates a **leakage spectrum** on the energy grid `leak_edges` into `leak_bins` — one `TallyBin` per energy bin, one Monte-Carlo realization per active generation, exactly like the track- length `tally`. · called at [L622](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L622) · *(calls below the depth limit not shown)*
      - [`reactor_physics.rs::Estimate::exact`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L101) `pub const fn exact(v: f64) -> Self` — A known-exact value (zero uncertainty). · called at [L640](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L640) · *(calls below the depth limit not shown)*
      - [`reactor_physics.rs::group_of`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L453) `fn group_of(e_hi: f64, cut_t: f64, cut_r: f64) -> Group` — Which group a fine bin `[e_lo, e_hi]` belongs to, given the two boundaries. · called at [L648](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L648) · *(calls below the depth limit not shown)*
      - [`reactor_physics.rs::bin_estimate`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L545) `fn bin_estimate(bin: &TallyBin, n: u64, per_source: f64) -> Estimate` — One tally bin → an `Estimate`, as a **per-source-neutron** rate: the per-generation mean over `n` active generations, divided by `per_source` (the histories per generation). · called at [L655](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L655) · *(calls below the depth limit not shown)*
      - [`reactor_physics.rs::Estimate::sum`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L115) `fn sum(parts: &[Estimate]) -> Estimate` — Sum of independent estimates: means add, variances add. · called at [L666](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L666) · *(calls below the depth limit not shown)*
      - `reactor_physics.rs::assemble_six_factors` *(expanded elsewhere in this walk)* · called at [L702](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L702)
      - [`mathf.rs::f64::r_ln`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/mathf.rs#L110) `fn r_ln(self) -> f64` · called at [L708](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L708) · *(calls below the depth limit not shown)*
      - [`reactor_physics.rs::Estimate::rel`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L106) `fn rel(self) -> f64` — Relative standard error `std / |mean|` (0 when `mean == 0`). · called at [L721](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L721) · *(calls below the depth limit not shown)*
    - UNRESOLVED(other): `clone` at [L192](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L192) (→ [`crates/outram-mc-libs/src/physics/reactor_physics.rs:229`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L229)) — resolves to `#[derive(Debug, Clone)]`, not a function body
    - `ugraphite.rs::Mix::densities` *(expanded elsewhere in this walk)* · called at [L199](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L199)
    - [`nuclide.rs::Nuclide::xs_at_energy`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L3038) `pub fn xs_at_energy(&self, e: f64, temp_k: f64) -> MicroXS` · called at [L211](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L211)
      - [`nuclide.rs::Nuclide::base_xs_at_energy`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L3157) `fn base_xs_at_energy(&self, e: f64, temp_k: f64) -> MicroXS` · called at [L3039](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L3039) · *(calls below the depth limit not shown)*
      - [`thermal.rs::ThermalScattering::total_xs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/thermal.rs#L1013) `pub fn total_xs(&self, e: f64) -> f64` — Total bound-atom thermal cross section \[barn per principal atom\] at incident energy `e` \[eV\] — σ_inel(E) + σ_el(E). · called at [L3053](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L3053) · *(calls below the depth limit not shown)*
    - [`reactor_physics.rs::assemble_six_factors`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L487) `pub fn assemble_six_factors(a: [Estimate; 3], a_thermal_fuel: Estimate, p: [Estimate; 3], l: [Estimate; 3], bounds: (f64, f64)) -> SixFactors` — Assemble the six factors from the group-resolved rates. · called at [L224](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L224)
      - `reactor_physics.rs::Estimate::sum` *(expanded elsewhere in this walk)* · called at [L494](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L494)
      - [`reactor_physics.rs::Estimate::ratio`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L135) `fn ratio(num: Estimate, den: Estimate) -> Estimate` — Ratio `num / den` of independent estimates: relative variances add. · called at [L504](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L504) · *(calls below the depth limit not shown)*
      - [`reactor_physics.rs::Estimate::product`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L151) `fn product(parts: &[Estimate]) -> Estimate` — Product of independent estimates: relative variances add. · called at [L512](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L512) · *(calls below the depth limit not shown)*
      - [`reactor_physics.rs::Estimate::diff`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L125) `fn diff(a: Estimate, b: Estimate) -> Estimate` — Difference `a − b` of independent estimates: variances add. · called at [L514](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L514) · *(calls below the depth limit not shown)*
  - [`ugraphite_common.rs::print_case`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L244) `pub fn print_case(label: &str, c_per_u: f64, r: &CaseResult)` — Print one case: k, the three-group factors, telescoping, the two-group view, the group rates and the fuel split. · called at [L261](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L261)
    - UNRESOLVED(closure): `e` at [L255](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L255) (→ [`crates/outram-mc-libs/examples/common/ugraphite_common.rs:247`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L247)) — call through a closure or fn-typed binding `e`
    - UNRESOLVED(closure): `e` at [L256](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L256) (→ [`crates/outram-mc-libs/examples/common/ugraphite_common.rs:247`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L247)) — call through a closure or fn-typed binding `e`
    - UNRESOLVED(closure): `e` at [L257](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L257) (→ [`crates/outram-mc-libs/examples/common/ugraphite_common.rs:247`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L247)) — call through a closure or fn-typed binding `e`
    - UNRESOLVED(closure): `e` at [L258](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L258) (→ [`crates/outram-mc-libs/examples/common/ugraphite_common.rs:247`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L247)) — call through a closure or fn-typed binding `e`
    - UNRESOLVED(closure): `e` at [L259](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L259) (→ [`crates/outram-mc-libs/examples/common/ugraphite_common.rs:247`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L247)) — call through a closure or fn-typed binding `e`
    - UNRESOLVED(closure): `e` at [L260](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L260) (→ [`crates/outram-mc-libs/examples/common/ugraphite_common.rs:247`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L247)) — call through a closure or fn-typed binding `e`
    - [`reactor_physics.rs::SixFactors::two_group_openmc_convention`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L291) `pub fn two_group_openmc_convention(&self) -> (f64, f64, f64, f64)` — The same run re-expressed in the **two-group** convention the OpenMC reference deck uses, for like-for-like comparison. · called at [L281](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L281)
  - [`ugraphite.rs::Mix::c_per_u`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/vv/ugraphite.rs#L80) `pub fn c_per_u(&self) -> f64` — Carbon atoms per uranium atom. · called at [L263](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L263)
  - [`ugraphite.rs::natural_mix`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/vv/ugraphite.rs#L181) `pub fn natural_mix(c_per_u: f64) -> Mix` — Natural uranium in graphite at 1.73 g/cm3 carbon, `c_per_u` carbon atoms per uranium atom. · called at [L266](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L266)
    - `ugraphite.rs::graphite_density` *(expanded elsewhere in this walk)* · called at [L182](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/vv/ugraphite.rs#L182)
  - UNRESOLVED(closure): `cell_r` at [L277](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L277) (→ [`crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs:221`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L221)) — call through a closure or fn-typed binding `cell_r`
  - UNRESOLVED(closure): `cell_r` at [L303](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L303) (→ [`crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs:221`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lumped_ugraphite_kinf.rs#L221)) — call through a closure or fn-typed binding `cell_r`
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
