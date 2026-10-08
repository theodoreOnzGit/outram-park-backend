# Uranium in graphite: why a reactor needs a moderator

> **Research, education and V&V only.** Not for reactor operation, licensing,
> safety decisions or emergency response
> ([`RESPONSIBLE_USE.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/RESPONSIBLE_USE.md)).
> **Review status: first draft, 2026-10-05, AI-assisted, not yet reviewed by a
> human.** Built from
> [`@@COMMIT_SHORT@@`](https://github.com/theodoreOnzGit/outram-park-backend/commit/@@COMMIT@@)
> on @@BUILD_DATE@@; every code link points at that commit.

<div class="mcw-demo" data-mc-widget="demo" data-src="../../demos/monte-carlo/?rung=ugraphite&amp;mode=watch" data-label="▶ Start the uranium-in-graphite demo here (Watch mode)"></div>

*The demo downloads five ENDF/B-VIII.0 tapes and graphite's thermal-scattering
law and processes them in your browser before the first neutron flies: allow a
minute or two. Each track is a real history from the transport code; chaining
them one after another is an illustration. Watch the colour: a neutron is born
fast (yellow-white), and it is slow and dark long before it is absorbed.*

## The problem

Godiva (rung 1) was **93.8 % U-235** by atoms (`vv::godiva`), and every one of
its fissions was caused by a fast neutron. Natural uranium is only
**0.72 % U-235** (the IUPAC composition used here, `vv::ugraphite::NAT_U`).
A lump of natural uranium alone never goes critical: its fast neutrons are
lost to U-238 before they find enough U-235. Yet the first reactors were
built from natural uranium and graphite.

> *History placeholder: the 1942 Chicago pile (natural uranium and uranium
> oxide in graphite) is the obvious hook here. No source on it is in the
> literature corpus yet (`crates/kovan-literature/CATALOGUE.md`, searched
> 2026-10-05), so nothing about it is stated as fact on this page. A sourced
> account, with page numbers, is waiting for the maintainer.*

**The question of this lesson:** *what does the graphite do?* Try the
simplest version first: uranium mixed **evenly** through graphite, an infinite
homogeneous mixture. (History did not do it this way. Step 7 shows why.)

The model is a cube of one mixture with **reflective walls**: a neutron that
reaches a wall comes back, so nothing leaks and the cube behaves as an infinite
medium. Its multiplication factor is $k_\infty$. The mixture of the main case is
the uranium and carbon of one HTR-10 fuel pebble (17 wt% U-235) smeared evenly:
**767.2 carbon atoms per uranium atom** ($N_C/N_U = 767.2$), computed from the
published pebble specification (`vv::ugraphite::htr10_pebble_mix`) and not
chosen to give any particular $k$.

The model the code runs is these few lines:

```rust,ignore
{{#include ../../../examples/ugraphite_four_factor.rs:model}}
```

([`ugraphite_four_factor.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#@@L:crates/outram-mc-libs/examples/ugraphite_four_factor.rs:anchor=model@@);
the compositions are in
[`vv::ugraphite`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/vv/ugraphite.rs#@@L:crates/outram-mc-libs/src/vv/ugraphite.rs:fn=htr10_pebble_mix@@).)

---

## 1. Why slow the neutrons down at all?

**Answer.** Because U-235 is far more willing to fission when the neutron is
slow. Its fission cross section rises roughly as $1/v$ as the neutron slows,
by orders of magnitude between a fast fission neutron and a thermal one, while
U-238 at thermal energies only captures weakly. Slow the neutrons down to
thermal energies, and the 0.72 % of U-235 can win against the 99.3 % of
U-238. Carbon is there to do the slowing, and itself absorbs very little.

The catch is the middle of the energy range. On the way down, U-238 has
**resonances**: narrow energies where its capture cross section spikes by
thousands of times. A neutron that happens to land on one is likely to be
captured. Seeing the curves is the best start: open the nuclear data demo's
reconstructed cross sections and look at U-235 fission, U-238 capture and
carbon scattering on a log-log scale.

<div class="mcw-demo" data-mc-widget="demo" data-src="../../demos/nuclear-data/?rung=reconr" data-label="▶ Look at the cross sections (nuclear data demo, RECONR)"></div>

*No numbers are quoted here on purpose: read them off the curves, which come
from our own processing of the ENDF/B-VIII.0 evaluations (the nuclear data
track explains where they come from).*

The same three curves sit beside the neutrons in this rung's own demo (above,
since 2026-10-05): the cross sections the browser processed for that run,
with a dot riding the current neutron's energy, so you can watch one slow
down past the resonances, or stop on one and be captured.

In the code, every cross section the transport uses is one call:
`Nuclide::xs_at_energy(e, temp_k)`, which reads the pointwise table that the
workspace's NJOY port reconstructed and Doppler-broadened from the ENDF tape.
Step 5 walks to it.

<div class="predict">

**Predict.** A 2 MeV neutron hits a carbon nucleus head-on. What fraction of
its energy can it lose in one collision: all of it, about a quarter, or about
2 %? And off a U-238 nucleus?

</div>

## 2. One collision with carbon

**Answer.** In an elastic collision with a nucleus of mass ratio $A$ (target
at rest), the neutron leaves with an energy $E'$ between $\alpha E$ and $E$:

$$\alpha = \left(\frac{A-1}{A+1}\right)^2$$

Where it lands in that range depends on the scattering angle in the
**centre-of-mass** frame, $\mu_{cm} = \cos\theta_{cm}$:

$$\frac{E'}{E} = \frac{(1+\alpha) + (1-\alpha)\,\mu_{cm}}{2}$$

so a head-on collision ($\mu_{cm} = -1$) gives $\alpha E$ and a grazing one
($\mu_{cm} = +1$) gives $E$. If $\mu_{cm}$ is uniform (isotropic in the centre
of mass), $E'/E$ is uniform on $[\alpha, 1]$.

| nuclide | AWR (tape) | $\alpha$ | most energy lost in one collision |
|---|---|---|---|
| H-1 | 0.9991673 | 1.7 × 10⁻⁷ | all of it |
| C-12 | 11.89365 | 0.7138 | 28.6 % |
| U-238 | 236.0058 | 0.9832 | 1.7 % |

*AWR from each ENDF/B-VIII.0 tape, the same table as rung 4's, computed
2026-10-04.*

<div class="mcw" data-mc-widget="collision" data-target="1"></div>

*Illustration (JavaScript's own random numbers). Left: velocity space. The
neutron arrives with velocity 1 along the grey line; after the collision its
velocity ends on the circle, centred on the centre-of-mass velocity. Right: the
lab energy bar $[\alpha E, E]$ and the histogram of $E'/E$, flat for isotropic
scattering. Try the two $\mu_{cm}$ buttons, and switch the target.*

**The code walk.** The transport loop (`transport_history_vr` in
`transport_csg.rs`, the same kernel LCT-008 runs) does not call a function
named "elastic scatter" for carbon. It samples the centre-of-mass cosine
inline, from the nuclide's ENDF angular law (MF=4) where the evaluation gives
one and isotropically ($\mu_{cm} = 2\xi - 1$) where it does not, then calls the
scattering kernel:

<div class="codewalk">

<!-- code-walk: from=crates/outram-mc-libs/src/physics/transport_csg.rs::transport_history_vr to=crates/outram-mc-libs/src/physics/scatter.rs::free_gas_elastic_scatter_dbrc depth=3 -->
<!-- generated by `kovan-cli code-walk-check --update`; edit the comment above, not this -->

Call chain from `transport_csg.rs::transport_history_vr` to `scatter.rs::free_gas_elastic_scatter_dbrc`: 1 hop, 1 shortest chain. Each step shows its code; the name links to it on GitHub.

**1.** [`transport_csg.rs::transport_history_vr`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1314) — One history of the CSG k-eigenvalue kernel, with an explicit variance-reduction configuration.

<!-- snippet-check: crates/outram-mc-libs/src/physics/transport_csg.rs:1314 fn transport_history_vr -->
<!-- snippet-check: crates/outram-mc-libs/src/physics/transport_csg.rs:2291 free_gas_elastic_scatter_dbrc -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/physics/transport_csg.rs:1314:1321}}
    // …
{{#include ../../../../../crates/outram-mc-libs/src/physics/transport_csg.rs:2289:2292}}
    // … (the rest of the function: follow the link above)
```

**2.** → [`scatter.rs::free_gas_elastic_scatter_dbrc`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/scatter.rs#L265) · called at [L2291](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2291) — `free_gas_elastic_scatter` with an optional **DBRC** correction.

<!-- snippet-check: crates/outram-mc-libs/src/physics/scatter.rs:265 fn free_gas_elastic_scatter_dbrc -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/physics/scatter.rs:265:304}}
    // … (the rest of the function: follow the link above)
```

Unresolved calls inside the functions on this chain:

- in [`transport_csg.rs::transport_history_vr`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1314):
  - UNRESOLVED(closure): `observe` at [L1542](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1542) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1362`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1362)) — call through a closure or fn-typed binding `observe`
  - UNRESOLVED(no-definition): `Some` at [L1580](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1580) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `as_deref_mut` at [L1580](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1580) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `record` at [L1581](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1581) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `observe` at [L1583](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1583) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `State` at [L1583](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1583) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `Some` at [L1597](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1597) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `as_ref` at [L1597](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1597) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `Some` at [L1598](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1598) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `look_up` at [L1598](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1598) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `apply_window` at [L1599](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1599) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `future_seed` at [L1618](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1618) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `push` at [L1619](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1619) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `Some` at [L1627](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1627) — rust-analyzer returned no definition
  - UNRESOLVED(macro): `track!` at [L1659](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1659) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L1668](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1668) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(closure): `observe` at [L1693](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1693) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1362`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1362)) — call through a closure or fn-typed binding `observe`
  - UNRESOLVED(closure): `observe` at [L1739](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1739) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1362`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1362)) — call through a closure or fn-typed binding `observe`
  - UNRESOLVED(closure): `observe` at [L1899](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1899) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1362`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1362)) — call through a closure or fn-typed binding `observe`
  - UNRESOLVED(macro): `track!` at [L1974](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1974) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2032](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2032) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2035](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2035) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2320](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2320) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `weight_window_checkpoint!` at [L2325](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2325) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1595`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1595)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2326](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2326) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2372](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2372) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2381](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2381) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `weight_window_checkpoint!` at [L2397](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2397) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1595`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1595)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2398](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2398) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2410](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2410) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
<!-- /code-walk -->

</div>

which, above the free-gas threshold, holds the target at rest and lands in
the two-body kinematics:

<div class="codewalk">

<!-- code-walk: from=crates/outram-mc-libs/src/physics/scatter.rs::free_gas_elastic_scatter_dbrc to=crates/outram-mc-libs/src/physics/scatter.rs::cm_to_lab depth=3 -->
<!-- generated by `kovan-cli code-walk-check --update`; edit the comment above, not this -->

Call chain from `scatter.rs::free_gas_elastic_scatter_dbrc` to `scatter.rs::cm_to_lab`: 2 hops, 1 shortest chain. Each step shows its code; the name links to it on GitHub.

**1.** [`scatter.rs::free_gas_elastic_scatter_dbrc`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/scatter.rs#L265) — `free_gas_elastic_scatter` with an optional **DBRC** correction.

<!-- snippet-check: crates/outram-mc-libs/src/physics/scatter.rs:265 fn free_gas_elastic_scatter_dbrc -->
<!-- snippet-check: crates/outram-mc-libs/src/physics/scatter.rs:292 two_body_scatter_with_mu -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/physics/scatter.rs:265:293}}
    // … (the rest of the function: follow the link above)
```

**2.** → [`scatter.rs::two_body_scatter_with_mu`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/scatter.rs#L178) · called at [L292](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/scatter.rs#L292) — Two-body scatter with a **caller-supplied** centre-of-mass scattering cosine `mu_cm` — the anisotropic form of `two_body_scatter`.

<!-- snippet-check: crates/outram-mc-libs/src/physics/scatter.rs:178 fn two_body_scatter_with_mu -->
<!-- snippet-check: crates/outram-mc-libs/src/physics/scatter.rs:189 cm_to_lab -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/physics/scatter.rs:178:190}}
    // … (the rest of the function: follow the link above)
```

**3.** → [`scatter.rs::cm_to_lab`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/scatter.rs#L127) · called at [L189](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/scatter.rs#L189)

<!-- snippet-check: crates/outram-mc-libs/src/physics/scatter.rs:127 fn cm_to_lab -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/physics/scatter.rs:127:138}}
```
<!-- /code-walk -->

</div>

```rust,ignore
{{#include ../../../src/physics/scatter.rs:cm_to_lab}}
```

([`scatter.rs`, `cm_to_lab`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/scatter.rs#@@L:crates/outram-mc-libs/src/physics/scatter.rs:anchor=cm_to_lab@@).)
With `e_cm_out` $= E A^2/(A+1)^2$ for elastic scattering, `e_out` is
$E (A^2 + 2A\mu_{cm} + 1)/(A+1)^2$: set `mu_cm` to −1 and it is exactly
$\alpha E$.

*Two footnotes.* Real carbon is not isotropic in the centre of mass at MeV
energies: the code uses the evaluation's MF=4 law, which is why the walk
passes through `sample_elastic_mu_cm`. And "target at rest" fails near
thermal energies, where the carbon atom's own motion matters: that is step 4.

<div class="predict">

**Predict.** About how many collisions with carbon does it take to bring a
2 MeV neutron down to thermal (0.025 eV): ten, a hundred, or a thousand?

</div>

## 3. Lethargy: counting the collisions

**Answer.** Energy is the wrong scale for slowing down: each collision removes
a *fraction* of the energy, not an amount. So measure progress in
**lethargy**, $u = \ln(E_0/E)$. Each collision then adds, on average, the
same amount of lethargy, whatever the energy:

$$\xi = \langle \ln(E/E') \rangle = 1 + \frac{\alpha \ln \alpha}{1-\alpha}$$

and the number of collisions to go from $E_0$ to $E$ is about

$$n \approx \frac{\ln(E_0/E)}{\xi}$$

From 2 MeV to 0.025 eV, $\ln(E_0/E) = 18.2$:

| nuclide | $\xi$ | collisions, 2 MeV → 0.025 eV |
|---|---|---|
| H-1 | 0.9999973 | 18.2 |
| C-12 | 0.1591 | **114** |
| U-238 | 0.00845 | 2153 |

<div class="mcw" data-mc-widget="slowdown" data-target="1"></div>

*Illustration (target at rest, isotropic). One neutron's energy against
collision number, on a log scale: a ragged staircase around the straight
dashed line of slope $\xi$. "1000 neutrons" gives the spread of the count.*

**There is no function in the code that computes $\xi$, lethargy, or "114
collisions".** They *emerge* from the transport loop repeating one collision.
What the code has is a **fork**, taken afresh at every scattering collision,
and it is the subject of the next step:

```rust,ignore
{{#include ../../../src/physics/transport_csg.rs:scatter_fork}}
```

([`transport_csg.rs`, the scatter fork](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#@@L:crates/outram-mc-libs/src/physics/transport_csg.rs:anchor=scatter_fork@@).)

**Measured check (verification).** `examples/epithermal_slowing_down.rs`
samples the transport's own collision kernel 200 000 times per energy and
compares $\xi$ and $\langle E'/E\rangle$ with the formulas above. Re-measured
2026-10-04 on `develop` at `bfeb81a083` (the record is in that example's doc
comment): 104 rows, eight nuclides, from 0.0253 eV to 10 keV, all inside the
envelope, worst $|\xi - \xi_0| = 1.08 \times 10^{-3}$ (Li-7 at 100 eV); for
graphite's C-12 between 6.674 eV and 10 keV, $\xi/\xi_0$ is 0.9972 to 0.9998.
Below about 5 eV the bound-atom law takes over and the formula no longer
applies.

<div class="predict">

**Predict.** If every collision holds the carbon atom still, a neutron can
only lose energy. After 400 collisions in graphite at 600 K, where does it
end up: at about $kT$ (0.05 eV), at a millionth of that, or at essentially
zero?

</div>

## 4. The bottom of the slope: the carbon atom moves

**Answer.** Near thermal energies the neutron is no faster than the atoms it
hits. A carbon atom at room temperature jiggles with an energy of order $kT$
(0.026 eV at 296 K), so a collision can **give** the neutron energy as well as
take it. The neutron population ends in balance with the moderator, a
Maxwellian at the moderator's temperature. Getting this wrong is not a small
error.

**4a. The bug this code once had.** Until bead `op-50vu` (September 2026),
the transport held every target at rest at every energy. Each collision on its
own was valid, but the sequence has **no equilibrium**: a neutron that can
only lose energy cools without limit.

**Measured, re-run for this page** (2026-10-05, `epithermal_slowing_down.rs`
with its explicit ablation `TARGET_AT_REST=1`, which puts the old kernel
back; 20 000 neutrons × 400 collisions from 1 eV at 600 K, where the correct
equilibrium of this walk is $\langle E\rangle = 2kT = 0.103$ eV):

| medium | $\langle E\rangle$ after 400 collisions |
|---|---|
| carbon, target held at rest | **1.5 × 10⁻²⁷ eV** |
| oxygen-16, target held at rest | 2.9 × 10⁻²¹ eV |
| carbon in graphite, S(α,β) | 0.102 eV ($\langle E\rangle/2kT = 0.989$) |

Twenty-six decades too cold. Each collision was right; the physics that was
missing was the target's own motion, and only a whole history shows it. With
the fix (below), free-gas carbon in the same walk ends at
$\langle E\rangle/2kT = 0.981$ (re-measured 2026-10-04, the example's doc
comment).

**4b. Free gas.** The fix samples the target nucleus's thermal velocity from
a Maxwellian at the material temperature, below $400\,kT$ (OpenMC's
`FREE_GAS_THRESHOLD`), and does the collision in the frame where the target
was moving. For U-238 the same function applies **DBRC** (Doppler-broadened
rejection), which samples the target velocity weighted by the resonance's own
shape so that a neutron near a resonance scatters correctly.

**4c. Graphite is not a gas: S(α,β).** Carbon in graphite is bound in a
crystal. It cannot recoil freely; it exchanges energy with the lattice in
**phonons**, and at low energy the crystal planes diffract neutrons
coherently (**Bragg edges**: sudden steps in the elastic cross section). The
evaluated **thermal scattering law** $S(\alpha,\beta)$ describes all of this,
and the code samples it below its cutoff (a few eV). The law is processed
from `tsl-crystalline-graphite.endf` by the workspace's THERMR port.

**The fork, in the order the code tests it:**

1. **S(α,β):** the nuclide carries a thermal law and $E$ is below its cutoff →
   `Nuclide::sample_thermal`, lab-frame energy and angle from the bound-atom
   law;
2. **free gas:** otherwise, below $400\,kT$ (and inside a resonant nuclide's
   DBRC window) → the target's motion is sampled;
3. **target at rest:** above that → step 2's two-body kinematics.

<div class="codewalk">

<!-- code-walk: from=crates/outram-mc-libs/src/physics/transport_csg.rs::transport_history_vr to=crates/outram-mc-libs/src/material/nuclide.rs::Nuclide::sample_thermal depth=3 -->
<!-- generated by `kovan-cli code-walk-check --update`; edit the comment above, not this -->

Call chain from `transport_csg.rs::transport_history_vr` to `nuclide.rs::Nuclide::sample_thermal`: 1 hop, 1 shortest chain. Each step shows its code; the name links to it on GitHub.

**1.** [`transport_csg.rs::transport_history_vr`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1314) — One history of the CSG k-eigenvalue kernel, with an explicit variance-reduction configuration.

<!-- snippet-check: crates/outram-mc-libs/src/physics/transport_csg.rs:1314 fn transport_history_vr -->
<!-- snippet-check: crates/outram-mc-libs/src/physics/transport_csg.rs:2275 sample_thermal -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/physics/transport_csg.rs:1314:1321}}
    // …
{{#include ../../../../../crates/outram-mc-libs/src/physics/transport_csg.rs:2273:2276}}
    // … (the rest of the function: follow the link above)
```

**2.** → [`nuclide.rs::Nuclide::sample_thermal`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L3480) · called at [L2275](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2275) — Sample a bound-atom S(α,β) thermal scatter at incident energy `e` \[eV\], returning `Some((e_out, mu_lab))` — a **laboratory-frame** outgoing energy \[eV\] and scattering cosine — when this nuclide carries a `ThermalScattering` table and `e` is below its cutoff, or `None` otherwise (the caller then falls back to free-gas / anisotropic elastic).

<!-- snippet-check: crates/outram-mc-libs/src/material/nuclide.rs:3480 fn sample_thermal -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/material/nuclide.rs:3480:3482}}
```

Unresolved calls inside the functions on this chain:

- in [`transport_csg.rs::transport_history_vr`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1314):
  - UNRESOLVED(closure): `observe` at [L1542](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1542) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1362`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1362)) — call through a closure or fn-typed binding `observe`
  - UNRESOLVED(no-definition): `Some` at [L1580](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1580) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `as_deref_mut` at [L1580](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1580) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `record` at [L1581](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1581) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `observe` at [L1583](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1583) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `State` at [L1583](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1583) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `Some` at [L1597](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1597) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `as_ref` at [L1597](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1597) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `Some` at [L1598](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1598) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `look_up` at [L1598](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1598) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `apply_window` at [L1599](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1599) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `future_seed` at [L1618](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1618) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `push` at [L1619](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1619) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `Some` at [L1627](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1627) — rust-analyzer returned no definition
  - UNRESOLVED(macro): `track!` at [L1659](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1659) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L1668](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1668) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(closure): `observe` at [L1693](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1693) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1362`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1362)) — call through a closure or fn-typed binding `observe`
  - UNRESOLVED(closure): `observe` at [L1739](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1739) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1362`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1362)) — call through a closure or fn-typed binding `observe`
  - UNRESOLVED(closure): `observe` at [L1899](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1899) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1362`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1362)) — call through a closure or fn-typed binding `observe`
  - UNRESOLVED(macro): `track!` at [L1974](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1974) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2032](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2032) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2035](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2035) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2320](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2320) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `weight_window_checkpoint!` at [L2325](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2325) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1595`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1595)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2326](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2326) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2372](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2372) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2381](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2381) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `weight_window_checkpoint!` at [L2397](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2397) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1595`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1595)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2398](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2398) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2410](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2410) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
<!-- /code-walk -->

</div>

<div class="codewalk">

<!-- code-walk: from=crates/outram-mc-libs/src/material/nuclide.rs::Nuclide::sample_thermal to=crates/outram-mc-libs/src/material/thermal.rs::ThermalScattering::sample depth=3 -->
<!-- generated by `kovan-cli code-walk-check --update`; edit the comment above, not this -->

Call chain from `nuclide.rs::Nuclide::sample_thermal` to `thermal.rs::ThermalScattering::sample`: 1 hop, 1 shortest chain. Each step shows its code; the name links to it on GitHub.

**1.** [`nuclide.rs::Nuclide::sample_thermal`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L3480) — Sample a bound-atom S(α,β) thermal scatter at incident energy `e` \[eV\], returning `Some((e_out, mu_lab))` — a **laboratory-frame** outgoing energy \[eV\] and scattering cosine — when this nuclide carries a `ThermalScattering` table and `e` is below its cutoff, or `None` otherwise (the caller then falls back to free-gas / anisotropic elastic).

<!-- snippet-check: crates/outram-mc-libs/src/material/nuclide.rs:3480 fn sample_thermal -->
<!-- snippet-check: crates/outram-mc-libs/src/material/nuclide.rs:3481 sample -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/material/nuclide.rs:3480:3482}}
```

**2.** → [`thermal.rs::ThermalScattering::sample`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/thermal.rs#L1080) · called at [L3481](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L3481) — Sample a thermal scatter at incident energy `e` \[eV\], returning `Some((e_out, mu_lab))` — a laboratory-frame outgoing energy \[eV\] and scattering cosine — or `None` at or above the cutoff.

<!-- snippet-check: crates/outram-mc-libs/src/material/thermal.rs:1080 fn sample -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/material/thermal.rs:1080:1098}}
```
<!-- /code-walk -->

</div>

The demo labels nothing by branch yet; in this graphite mixture every
collision below a few eV takes branch 1, carbon above it branch 3, and U-238
near its resonances branch 2.

**Measured checks (verification against NJOY2016's THERMR,** re-measured
2026-10-04 on `develop` at `bfeb81a083`; the records are in the examples'
doc comments**):**

- graphite cross sections, `graphite_vs_njoy_thermr.rs`: incoherent inelastic
  worst **+0.060 %** (at 1 meV), coherent elastic worst **−0.085 %** (at
  3.0 eV), and exactly zero below the first Bragg edge, as it must be;
- the sampled outgoing energy, `graphite_kernel_vs_njoy_thermr.rs`:
  $\langle E'\rangle/E$ within **+0.30 %** at 0.01 eV (inside its 1σ of
  0.35 %) and closer above;
- the sampled angle, `graphite_sab_angle_and_width_vs_njoy_thermr.rs`:
  $\bar\mu$ worst **−0.0040** (inelastic at 5 meV); the outgoing-energy
  **width** is narrower than NJOY's at every energy, worst **−1.14 %** at
  2.6 meV. That is a known, one-signed, open defect of the tabulation, kept
  inside a 4 % envelope.
- the thermal equilibrium of the walk with S(α,β) in graphite at 600 K,
  `epithermal_slowing_down.rs`: $\langle E\rangle / 2kT = 0.9889$ after 400
  collisions (the fixed point of a walk over collisions is $2kT$).

**What is S(α,β) worth in $k$?** Run the main case again with the graphite law switched off (the example's
explicit ablation `GRAPHITE_SAB=0`, carbon treated as a free gas), at the
record's own size (2026-10-05, the example's doc comment): $k_\infty$ =
1.57038 ± 0.00075 without it, against 1.56777 ± 0.00081 with it. **The
bound-atom law is worth −261 ± 110 pcm here (2.4σ)**. Which of the four
factors carries it is not resolved: each moves by 0.1 % or less. A quarter of
a per cent: small
beside the 30 % that resonance capture costs, and exactly the kind of term a
code that "matches" without it would be hiding.

<div class="predict">

**Predict.** U-238's biggest resonance, at 6.67 eV, has a capture cross
section of thousands of barns at its peak. If you double the amount of U-238
in the mixture, does resonance capture double?

</div>

## 5. Resonances, and the probability of escaping them

**Answer.** On its way down, a neutron's energy steps down by a random
fraction each collision, so it can land on a resonance peak. The probability
of getting past all of them is the **resonance escape probability** $p$.
With little U-238 (very dilute), the capture adds up over the resonances as
the **resonance integral**,

$$RI_\infty = \int \sigma_\gamma(E)\,\frac{dE}{E}$$

and with $N_{238}$ U-238 atoms per cm³ among moderator atoms that scatter
with $\Sigma_s$,

$$p \approx \exp\left(-\frac{N_{238}\,RI_{\text{eff}}}{\xi \Sigma_s}\right)$$

Why $RI_{\text{eff}}$ and not $RI_\infty$: when there is a lot of U-238, the
neutrons at a resonance's peak energy are absorbed so quickly that the flux
there **dips** (a notch in the flux spectrum). Fewer neutrons are there to be
captured than the dilute formula assumes. That is **self-shielding in
energy**, and it is why doubling the U-238 does not double the capture.

**There is no function that computes $p$.** It emerges, like $\xi$. Each
collision picks a nuclide in proportion to its $N\sigma_t$ at the neutron's
energy, then a reaction in proportion to that nuclide's cross sections; at a
resonance U-238's capture dominates both choices. In U-238's **unresolved**
resonance range (above about 20 keV, where the resonances are too dense to
measure one by one) the cross sections come from **probability tables** (URR),
sampled once per neutron per energy:

<div class="codewalk">

<!-- code-walk: from=crates/outram-mc-libs/src/physics/transport_csg.rs::transport_history_vr to=crates/outram-mc-libs/src/material/material.rs::Material::sample_nuclide_urr depth=3 -->
<!-- generated by `kovan-cli code-walk-check --update`; edit the comment above, not this -->

Call chain from `transport_csg.rs::transport_history_vr` to `material.rs::Material::sample_nuclide_urr`: 1 hop, 1 shortest chain. Each step shows its code; the name links to it on GitHub.

**1.** [`transport_csg.rs::transport_history_vr`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1314) — One history of the CSG k-eigenvalue kernel, with an explicit variance-reduction configuration.

<!-- snippet-check: crates/outram-mc-libs/src/physics/transport_csg.rs:1314 fn transport_history_vr -->
<!-- snippet-check: crates/outram-mc-libs/src/physics/transport_csg.rs:1897 sample_nuclide_urr -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/physics/transport_csg.rs:1314:1321}}
    // …
{{#include ../../../../../crates/outram-mc-libs/src/physics/transport_csg.rs:1895:1898}}
    // … (the rest of the function: follow the link above)
```

**2.** → [`material.rs::Material::sample_nuclide_urr`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/material.rs#L186) · called at [L1897](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1897) — `Self::sample_nuclide` with each nuclide weighted by its **band** total (see `Self::macro_xs_total_urr`), as OpenMC's `sample_nuclide` weights by the same cached micro total it flew on.

<!-- snippet-check: crates/outram-mc-libs/src/material/material.rs:186 fn sample_nuclide_urr -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/material/material.rs:186:215}}
```

Unresolved calls inside the functions on this chain:

- in [`transport_csg.rs::transport_history_vr`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1314):
  - UNRESOLVED(closure): `observe` at [L1542](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1542) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1362`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1362)) — call through a closure or fn-typed binding `observe`
  - UNRESOLVED(no-definition): `Some` at [L1580](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1580) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `as_deref_mut` at [L1580](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1580) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `record` at [L1581](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1581) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `observe` at [L1583](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1583) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `State` at [L1583](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1583) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `Some` at [L1597](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1597) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `as_ref` at [L1597](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1597) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `Some` at [L1598](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1598) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `look_up` at [L1598](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1598) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `apply_window` at [L1599](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1599) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `future_seed` at [L1618](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1618) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `push` at [L1619](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1619) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `Some` at [L1627](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1627) — rust-analyzer returned no definition
  - UNRESOLVED(macro): `track!` at [L1659](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1659) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L1668](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1668) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(closure): `observe` at [L1693](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1693) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1362`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1362)) — call through a closure or fn-typed binding `observe`
  - UNRESOLVED(closure): `observe` at [L1739](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1739) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1362`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1362)) — call through a closure or fn-typed binding `observe`
  - UNRESOLVED(closure): `observe` at [L1899](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1899) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1362`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1362)) — call through a closure or fn-typed binding `observe`
  - UNRESOLVED(macro): `track!` at [L1974](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1974) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2032](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2032) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2035](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2035) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2320](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2320) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `weight_window_checkpoint!` at [L2325](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2325) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1595`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1595)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2326](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2326) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2372](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2372) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2381](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2381) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `weight_window_checkpoint!` at [L2397](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2397) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1595`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1595)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2398](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2398) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2410](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2410) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
<!-- /code-walk -->

</div>

<div class="codewalk">

<!-- code-walk: from=crates/outram-mc-libs/src/physics/transport_csg.rs::transport_history_vr to=crates/outram-mc-libs/src/material/nuclide.rs::Nuclide::xs_at_energy_urr depth=3 -->
<!-- generated by `kovan-cli code-walk-check --update`; edit the comment above, not this -->

Call chain from `transport_csg.rs::transport_history_vr` to `nuclide.rs::Nuclide::xs_at_energy_urr`: 1 hop, 1 shortest chain. Each step shows its code; the name links to it on GitHub.

**1.** [`transport_csg.rs::transport_history_vr`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1314) — One history of the CSG k-eigenvalue kernel, with an explicit variance-reduction configuration.

<!-- snippet-check: crates/outram-mc-libs/src/physics/transport_csg.rs:1314 fn transport_history_vr -->
<!-- snippet-check: crates/outram-mc-libs/src/physics/transport_csg.rs:1911 xs_at_energy_urr -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/physics/transport_csg.rs:1314:1321}}
    // …
{{#include ../../../../../crates/outram-mc-libs/src/physics/transport_csg.rs:1909:1912}}
    // … (the rest of the function: follow the link above)
```

**2.** → [`nuclide.rs::Nuclide::xs_at_energy_urr`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L1530) · called at [L1911](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1911) — This nuclide's microscopic cross sections at `e` \[eV\] and `temp_k` \[K\], **with unresolved-resonance self-shielding applied** from the sampled band `xi`.

<!-- snippet-check: crates/outram-mc-libs/src/material/nuclide.rs:1530 fn xs_at_energy_urr -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/material/nuclide.rs:1530:1560}}
```

Unresolved calls inside the functions on this chain:

- in [`transport_csg.rs::transport_history_vr`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1314):
  - UNRESOLVED(closure): `observe` at [L1542](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1542) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1362`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1362)) — call through a closure or fn-typed binding `observe`
  - UNRESOLVED(no-definition): `Some` at [L1580](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1580) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `as_deref_mut` at [L1580](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1580) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `record` at [L1581](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1581) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `observe` at [L1583](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1583) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `State` at [L1583](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1583) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `Some` at [L1597](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1597) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `as_ref` at [L1597](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1597) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `Some` at [L1598](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1598) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `look_up` at [L1598](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1598) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `apply_window` at [L1599](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1599) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `future_seed` at [L1618](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1618) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `push` at [L1619](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1619) — rust-analyzer returned no definition
  - UNRESOLVED(no-definition): `Some` at [L1627](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1627) — rust-analyzer returned no definition
  - UNRESOLVED(macro): `track!` at [L1659](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1659) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L1668](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1668) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(closure): `observe` at [L1693](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1693) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1362`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1362)) — call through a closure or fn-typed binding `observe`
  - UNRESOLVED(closure): `observe` at [L1739](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1739) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1362`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1362)) — call through a closure or fn-typed binding `observe`
  - UNRESOLVED(closure): `observe` at [L1899](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1899) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1362`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1362)) — call through a closure or fn-typed binding `observe`
  - UNRESOLVED(macro): `track!` at [L1974](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1974) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2032](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2032) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2035](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2035) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2320](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2320) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `weight_window_checkpoint!` at [L2325](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2325) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1595`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1595)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2326](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2326) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2372](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2372) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2381](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2381) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `weight_window_checkpoint!` at [L2397](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2397) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1595`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1595)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2398](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2398) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
  - UNRESOLVED(macro): `track!` at [L2410](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L2410) (→ [`crates/outram-mc-libs/src/physics/transport_csg.rs:1568`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L1568)) — workspace macro; its expansion is not followed
<!-- /code-walk -->

</div>

`xs_at_energy_urr` falls through to `xs_at_energy` outside the unresolved
range, and `xs_at_energy` reads the resonances RECONR reconstructed (the
nuclear data track's
[RECONR rung](../../deep-dives/nuclear-data/reconr.html)).

**Measured check (verification against an analytic limit).**
`examples/u238_resonance_escape.rs` follows neutrons in energy only (no
geometry) through U-238 in carbon, with the transport's own collision physics,
and turns the absorbed fraction into $RI_{\text{eff}}$. Re-measured 2026-10-04
(record in that example's doc comment), at this lesson's 296 K, with DBRC and
URR on (the transport's defaults), as a fraction of $RI_\infty = 274.637$ b
(from our own data, checked against NJOY by `u238_resonance_integral.rs`):

| carbon per U-238, as $\sigma_0$ (b) | $RI_{\text{eff}} / RI_\infty$ |
|---|---|
| 379 580 (trace U-238) | 0.982 ± 0.011 |
| 37 958 | 0.903 |
| 3 796 | 0.524 |
| 380 | 0.188 |

At trace U-238 it is 1 within 1.7σ: the dilute limit. As the U-238 grows,
self-shielding cuts the effective integral to a fifth. (The commonly quoted
experimental value of the resonance integral is **not** used here: no source
for it was found, see `u238_resonance_integral.rs`.)

<div class="predict">

**Predict.** For every 100 neutrons born in the main-case mixture (17 wt%
U-235), how many get past the resonances to thermal energies: 50, 70 or 95?
And for every thermal neutron absorbed, how many new neutrons come out?

</div>

## 6. The four-factor formula: a neutron's life, in four ratios

**Answer.** Follow one generation in an infinite medium (nothing leaks).
Split the life of a neutron at two energies (100 keV and the cadmium cutoff
0.625 eV) and count:

$$k_\infty = \eta \, f \, p \, \varepsilon$$

- $\varepsilon$, **fast fission factor**: all fission neutrons divided by
  those from thermal fissions. The bonus Godiva set up: a fast neutron
  occasionally fissions before it slows down.
- $p$, **resonance escape probability**: the fraction that reaches thermal
  energies instead of being absorbed on the way.
- $f$, **thermal utilisation**: the fraction of thermal absorptions that
  happen in uranium rather than in carbon.
- $\eta$: neutrons produced per thermal absorption in uranium.

**The code** gets all four from one run's **track-length** tallies (the flux
summed over every flight, times the cross section: a different estimator from
rung 1's counting, and why it is used is a later rung's question), absorption
and production in three energy groups, then:

```rust,ignore
{{#include ../../../src/physics/reactor_physics.rs:six_factors}}
```

([`reactor_physics.rs`, `assemble_six_factors`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#@@L:crates/outram-mc-libs/src/physics/reactor_physics.rs:anchor=six_factors@@).)
With nothing leaking, $P_{FNL} = P_{TNL} = 1$ and the six factors are the
four. The product **telescopes**: every intermediate count cancels, so
$\eta f p \varepsilon$ is exactly total production over total absorption,
whatever the energy boundaries. The factors depend on the convention (where
you split the energy range, and what you call "fuel"); $k$ does not.

**One subtlety the homogeneous mixture forces.** The library calls a
*material* "fuel" when it holds uranium. Here the only material holds
uranium, so $f$ would be exactly 1. The example therefore splits the thermal
absorption by **nuclide** from the run's own fine-group flux, and hands the
library that split (its module docs say how, and print a binning check).

<div class="codewalk">

<!-- code-walk: from=crates/outram-mc-libs/examples/ugraphite_four_factor.rs::main to=crates/outram-mc-libs/src/physics/transport_csg.rs::run_keff_csg_reactor_physics depth=6 -->
<!-- generated by `kovan-cli code-walk-check --update`; edit the comment above, not this -->

Call chain from `ugraphite_four_factor.rs::main` to `transport_csg.rs::run_keff_csg_reactor_physics`: 5 hops, 2 shortest chains. Each step shows its code; the name links to it on GitHub.

**1.** [`ugraphite_four_factor.rs::main`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L300)

<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:300 fn main -->
<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:317 sweep -->
<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:318 main_case -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/examples/ugraphite_four_factor.rs:300:319}}
    // … (the rest of the function: follow the link above)
```

**2.** → [`ugraphite_four_factor.rs::sweep`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L378) · called at [L317](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L317)

<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:378 fn sweep -->
<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:391 run_homogeneous -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/examples/ugraphite_four_factor.rs:378:392}}
    // … (the rest of the function: follow the link above)
```

**8.** → [`ugraphite_four_factor.rs::run_homogeneous`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L293) · called at [L391](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L391) — The homogeneous medium: one material filling the reflective cube.

<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:293 fn run_homogeneous -->
<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:296 run_case -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/examples/ugraphite_four_factor.rs:293:297}}
```

**9.** → [`ugraphite_common.rs::run_case`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L170) · called at [L296](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L296) — Run the power iteration with six-factor capture on `geom` and assemble the factors with the requested fuel definition.

<!-- snippet-check: crates/outram-mc-libs/examples/common/ugraphite_common.rs:170 fn run_case -->
<!-- snippet-check: crates/outram-mc-libs/examples/common/ugraphite_common.rs:187 run_keff_reactor_physics -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/examples/common/ugraphite_common.rs:170:188}}
    // … (the rest of the function: follow the link above)
```

**10.** → [`reactor_physics.rs::run_keff_reactor_physics`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L575) · called at [L187](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L187) — Run a k-eigenvalue power iteration over `geom` and capture the six-factor decomposition and the lethargy-normalised flux spectrum from one combined track-length tally plus explicit leakage accounting.

<!-- snippet-check: crates/outram-mc-libs/src/physics/reactor_physics.rs:575 fn run_keff_reactor_physics -->
<!-- snippet-check: crates/outram-mc-libs/src/physics/reactor_physics.rs:622 run_keff_csg_reactor_physics -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/physics/reactor_physics.rs:575:580}}
    // …
{{#include ../../../../../crates/outram-mc-libs/src/physics/reactor_physics.rs:620:623}}
    // … (the rest of the function: follow the link above)
```

**11.** → [`transport_csg.rs::run_keff_csg_reactor_physics`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L578) · called at [L622](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L622) — Like `run_keff_csg`, but also accumulates a **leakage spectrum** on the energy grid `leak_edges` into `leak_bins` — one `TallyBin` per energy bin, one Monte-Carlo realization per active generation, exactly like the track- length `tally`.

<!-- snippet-check: crates/outram-mc-libs/src/physics/transport_csg.rs:578 fn run_keff_csg_reactor_physics -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/physics/transport_csg.rs:578:617}}
    // … (the rest of the function: follow the link above)
```

**7.** (from step 1) → [`ugraphite_four_factor.rs::main_case`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L322) · called at [L318](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L318)

<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:322 fn main_case -->
<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:331 run_homogeneous -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/examples/ugraphite_four_factor.rs:322:332}}
    // … (the rest of the function: follow the link above)
```

**8.** → [`ugraphite_four_factor.rs::run_homogeneous`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L293) · called at [L331](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L331) — The homogeneous medium: one material filling the reflective cube.

<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:293 fn run_homogeneous -->
<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:296 run_case -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/examples/ugraphite_four_factor.rs:293:297}}
```

**9.** → [`ugraphite_common.rs::run_case`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L170) · called at [L296](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L296) — Run the power iteration with six-factor capture on `geom` and assemble the factors with the requested fuel definition.

<!-- snippet-check: crates/outram-mc-libs/examples/common/ugraphite_common.rs:170 fn run_case -->
<!-- snippet-check: crates/outram-mc-libs/examples/common/ugraphite_common.rs:187 run_keff_reactor_physics -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/examples/common/ugraphite_common.rs:170:188}}
    // … (the rest of the function: follow the link above)
```

**10.** → [`reactor_physics.rs::run_keff_reactor_physics`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L575) · called at [L187](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L187) — Run a k-eigenvalue power iteration over `geom` and capture the six-factor decomposition and the lethargy-normalised flux spectrum from one combined track-length tally plus explicit leakage accounting.

<!-- snippet-check: crates/outram-mc-libs/src/physics/reactor_physics.rs:575 fn run_keff_reactor_physics -->
<!-- snippet-check: crates/outram-mc-libs/src/physics/reactor_physics.rs:622 run_keff_csg_reactor_physics -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/physics/reactor_physics.rs:575:580}}
    // …
{{#include ../../../../../crates/outram-mc-libs/src/physics/reactor_physics.rs:620:623}}
    // … (the rest of the function: follow the link above)
```

**11.** → [`transport_csg.rs::run_keff_csg_reactor_physics`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/transport_csg.rs#L578) · called at [L622](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L622) — Like `run_keff_csg`, but also accumulates a **leakage spectrum** on the energy grid `leak_edges` into `leak_bins` — one `TallyBin` per energy bin, one Monte-Carlo realization per active generation, exactly like the track- length `tally`.

<!-- snippet-check: crates/outram-mc-libs/src/physics/transport_csg.rs:578 fn run_keff_csg_reactor_physics -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/physics/transport_csg.rs:578:617}}
    // … (the rest of the function: follow the link above)
```

Unresolved calls inside the functions on this chain:

- in [`ugraphite_common.rs::run_case`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L170):
  - UNRESOLVED(other): `clone` at [L192](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L192) (→ [`crates/outram-mc-libs/src/physics/reactor_physics.rs:229`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L229)) — resolves to `#[derive(Debug, Clone)]`, not a function body
- in [`reactor_physics.rs::run_keff_reactor_physics`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L575):
  - UNRESOLVED(other): `default` at [L617](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L617) (→ [`crates/outram-mc-libs/src/tally/tally.rs:73`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/tally/tally.rs#L73)) — resolves to `#[derive(Debug, Default, Clone, PartialEq)]`, not a function body
  - UNRESOLVED(other): `default` at [L619](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L619) (→ [`crates/outram-mc-libs/src/tally/tally.rs:73`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/tally/tally.rs#L73)) — resolves to `#[derive(Debug, Default, Clone, PartialEq)]`, not a function body
<!-- /code-walk -->

</div>

<div class="codewalk">

<!-- code-walk: from=crates/outram-mc-libs/examples/ugraphite_four_factor.rs::main to=crates/outram-mc-libs/src/physics/reactor_physics.rs::assemble_six_factors depth=6 -->
<!-- generated by `kovan-cli code-walk-check --update`; edit the comment above, not this -->

Call chain from `ugraphite_four_factor.rs::main` to `reactor_physics.rs::assemble_six_factors`: 4 hops, 2 shortest chains. Each step shows its code; the name links to it on GitHub.

**1.** [`ugraphite_four_factor.rs::main`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L300)

<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:300 fn main -->
<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:317 sweep -->
<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:318 main_case -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/examples/ugraphite_four_factor.rs:300:319}}
    // … (the rest of the function: follow the link above)
```

**2.** → [`ugraphite_four_factor.rs::sweep`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L378) · called at [L317](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L317)

<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:378 fn sweep -->
<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:391 run_homogeneous -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/examples/ugraphite_four_factor.rs:378:392}}
    // … (the rest of the function: follow the link above)
```

**7.** → [`ugraphite_four_factor.rs::run_homogeneous`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L293) · called at [L391](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L391) — The homogeneous medium: one material filling the reflective cube.

<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:293 fn run_homogeneous -->
<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:296 run_case -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/examples/ugraphite_four_factor.rs:293:297}}
```

**8.** → [`ugraphite_common.rs::run_case`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L170) · called at [L296](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L296) — Run the power iteration with six-factor capture on `geom` and assemble the factors with the requested fuel definition.

<!-- snippet-check: crates/outram-mc-libs/examples/common/ugraphite_common.rs:170 fn run_case -->
<!-- snippet-check: crates/outram-mc-libs/examples/common/ugraphite_common.rs:224 assemble_six_factors -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/examples/common/ugraphite_common.rs:170:177}}
    // …
{{#include ../../../../../crates/outram-mc-libs/examples/common/ugraphite_common.rs:222:225}}
    // … (the rest of the function: follow the link above)
```

**9.** → [`reactor_physics.rs::assemble_six_factors`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L487) · called at [L224](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L224) — Assemble the six factors from the group-resolved rates.

<!-- snippet-check: crates/outram-mc-libs/src/physics/reactor_physics.rs:487 fn assemble_six_factors -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/physics/reactor_physics.rs:487:526}}
    // … (the rest of the function: follow the link above)
```

**6.** (from step 1) → [`ugraphite_four_factor.rs::main_case`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L322) · called at [L318](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L318)

<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:322 fn main_case -->
<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:331 run_homogeneous -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/examples/ugraphite_four_factor.rs:322:332}}
    // … (the rest of the function: follow the link above)
```

**7.** → [`ugraphite_four_factor.rs::run_homogeneous`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L293) · called at [L331](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L331) — The homogeneous medium: one material filling the reflective cube.

<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:293 fn run_homogeneous -->
<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:296 run_case -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/examples/ugraphite_four_factor.rs:293:297}}
```

**8.** → [`ugraphite_common.rs::run_case`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L170) · called at [L296](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L296) — Run the power iteration with six-factor capture on `geom` and assemble the factors with the requested fuel definition.

<!-- snippet-check: crates/outram-mc-libs/examples/common/ugraphite_common.rs:170 fn run_case -->
<!-- snippet-check: crates/outram-mc-libs/examples/common/ugraphite_common.rs:224 assemble_six_factors -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/examples/common/ugraphite_common.rs:170:177}}
    // …
{{#include ../../../../../crates/outram-mc-libs/examples/common/ugraphite_common.rs:222:225}}
    // … (the rest of the function: follow the link above)
```

**9.** → [`reactor_physics.rs::assemble_six_factors`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L487) · called at [L224](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L224) — Assemble the six factors from the group-resolved rates.

<!-- snippet-check: crates/outram-mc-libs/src/physics/reactor_physics.rs:487 fn assemble_six_factors -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/physics/reactor_physics.rs:487:526}}
    // … (the rest of the function: follow the link above)
```

Unresolved calls inside the functions on this chain:

- in [`ugraphite_common.rs::run_case`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L170):
  - UNRESOLVED(other): `clone` at [L192](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L192) (→ [`crates/outram-mc-libs/src/physics/reactor_physics.rs:229`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L229)) — resolves to `#[derive(Debug, Clone)]`, not a function body
<!-- /code-walk -->

</div>

**The result to quote** (main case, 17 wt%, $N_C/N_U = 767.2$, 296 K,
recorded 2026-10-04 at `c199b7dc1f` in the doc comment of
[`ugraphite_four_factor.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#@@L:crates/outram-mc-libs/examples/ugraphite_four_factor.rs:text=Main+case+(2026-10-04@@)
and in `verification_and_validation/tutorial_rung2/README.md`; 20 000
neutrons × [20 + 100] generations):

| | value |
|---|---|
| $k_\infty$ (power iteration) | **1.56777 ± 0.00081** |
| $\eta$ | 2.02809 ± 0.00132 |
| $f$ | 0.97470 ± 0.00063 |
| $p$ | 0.71398 ± 0.00044 |
| $\varepsilon$ | 1.11144 ± 0.00093 |
| $\eta f p \varepsilon$ | 1.56867 ± 0.00256 (= production / absorption to $2 \times 10^{-16}$) |

- **The prediction held.** Written before the run: $k_\infty \approx 1.4$–1.6,
  with $\eta \approx 2.0$, $f \approx 0.98$, $p \approx 0.70$,
  $\varepsilon \approx 1.0$–1.1. Measured 1.568, each factor inside its range.
- **$p$ against the energy-only walk of step 5** at the same composition:
  0.71274 ± 0.00072, against the transport's 0.71398 ± 0.00044 (1.5σ). They are
  not quite the same quantity (the transport's resonance group also holds the
  ~1 % of fission neutrons born below 100 keV), so agreement at this level is
  what is expected.
- **Against another code (verification, not validation; no experiment exists
  for a homogeneous mixture):** OpenMC 0.16.1-dev25 on the openmc.org
  ENDF/B-VIII.0 library, same mixture and histories: $k$ 1.56711 ± 0.00134,
  a difference of **+66 ± 157 pcm (0.4σ)**, and every factor within 0.15 %.
  The deck is committed:
  `verification_and_validation/tutorial_rung2/openmc_inputs/ugraphite_openmc.py`.
- The ± on $k$ is the standard error over the 100 active generations. Why
  that can be trusted, and when it cannot, is a later rung.

**Even at 17 % enrichment, 29 % of the neutrons are lost to the resonances**
($p = 0.714$). With natural uranium there is 24 times less U-235 per U-238.

<div class="predict">

**Predict, before reading on.** Natural uranium, mixed evenly into graphite.
Can *any* ratio of carbon to uranium reach $k_\infty = 1$? If you add more
carbon, which factor improves, and which gets worse?

</div>

## 7. Why nobody built it this way

**The expectation, written down before the run** (2026-10-04, in the
example's doc comment, commit `c199b7dc1`, not edited since): homogeneous
natural uranium in graphite **never reaches $k_\infty = 1$ at any ratio**,
because U-238 resonance capture is too strong. With little carbon, neutrons
reach the resonances with too few collisions in between and $p$ is small;
with a lot of carbon, carbon's own capture takes the thermal neutrons and $f$
falls. So $k_\infty$ rises, peaks and falls. The hand estimate put the
**maximum at about 0.75–0.8, near $N_C/N_U \approx 500$–800**.

**What the code gives** (2026-10-05, recorded in the doc comment of
`ugraphite_four_factor.rs` and in `verification_and_validation/tutorial_rung2/README.md`;
5000 neutrons × [20 + 50] generations per point, so each $k$ is good to about
±0.002):

| $N_C/N_U$ | $k_\infty$ | $\eta$ | $f$ | $p$ | $\varepsilon$ |
|---|---|---|---|---|---|
| 50 | 0.49117 ± 0.00185 | 1.31230 | 0.97457 | 0.30725 | 1.23959 |
| 100 | 0.61994 ± 0.00197 | 1.32213 | 0.95077 | 0.45311 | 1.09216 |
| 200 | 0.72553 ± 0.00229 | 1.32821 | 0.90663 | 0.58182 | 1.03976 |
| 300 | 0.76517 ± 0.00169 | 1.33054 | 0.86645 | 0.64808 | 1.02534 |
| 400 | **0.77850 ± 0.00217** | 1.33178 | 0.82970 | 0.69192 | 1.01853 |
| 500 | 0.77774 ± 0.00254 | 1.33253 | 0.79594 | 0.72371 | 1.01491 |
| 600 | 0.77153 ± 0.00243 | 1.33304 | 0.76483 | 0.74684 | 1.01248 |
| 800 | 0.74798 ± 0.00213 | 1.33368 | 0.70937 | 0.78376 | 1.00942 |
| 1000 | 0.71898 ± 0.00219 | 1.33410 | 0.66142 | 0.80868 | 1.00772 |
| 1500 | 0.64640 ± 0.00196 | 1.33464 | 0.56580 | 0.85134 | 1.00559 |
| 2500 | 0.52621 ± 0.00193 | 1.33507 | 0.43890 | 0.89470 | 1.00400 |

- **It never reaches 1.** The best ratio gives $k_\infty = 0.7785 \pm 0.0022$,
  about a hundred standard deviations short. The prediction held.
- **The shape is the predicted one, and the factors say why.** Going right,
  $p$ climbs from 0.31 to 0.89 (more carbon, more collisions between
  resonances), while $f$ falls from 0.97 to 0.44 (more carbon, more of the
  thermal neutrons absorbed in carbon). $\eta$ is natural uranium's, 1.31 to
  1.34 everywhere: far below the 17 % mixture's 2.03.
- **The maximum sits a little lower than predicted.** The top is flat between
  400 and 500 carbon atoms per uranium atom (0.7785 and 0.7777, the same
  within statistics), already 2σ lower at 600, and 0.748 at 800. The
  prediction said 500–800: partly refuted, and recorded as such.
- **A surprise:** at very little carbon ($N_C/N_U = 50$), $\varepsilon$ is
  1.24. The uranium is dense, most neutrons are absorbed before they are
  thermal, and fast fission of U-238 is a large share of what fissions remain.

**Not yet checked against another code.** The OpenMC deck that verified the
main case runs the sweep unchanged (`ugraphite_openmc.py --case natural --cu
R`); it was not available on the machine that ran the sweep, so that
comparison is pending.

**So the graphite does two jobs that pull against each other** in a
homogeneous mixture: more of it slows the neutrons past the resonances
(higher $p$), and more of it absorbs them once they are slow (lower $f$). The
way out is not a better ratio. It is to **stop mixing**: gather the uranium
into lumps, so that the U-238 shields itself in *space*. That is the
[next rung](lumped.md), where the same natural uranium, gathered into 1 cm
lumps at 200 carbon atoms per uranium atom, reaches $k_\infty = 1.090 \pm 0.002$.

**Run it yourself.** On your own machine, from a clone of the repository:

```text
cargo run --release -p outram-mc-libs --features endf-pebble-cases --example ugraphite_four_factor
MODE=sweep cargo run --release -p outram-mc-libs --features endf-pebble-cases --example ugraphite_four_factor
```

The first runs the main case and the $p$ check; the second the natural-uranium
sweep (`RATIOS=300,600,1000` picks your own points; `PARTICLES`, `INACTIVE`,
`ACTIVE`, `THREADS` set the size).

**Modify.** `CU=400` runs the main case's 17 wt% uranium at another ratio.
Predict first: does $k_\infty$ go up or down, and which factor moves most?

**Create.** Find the enrichment at which a homogeneous mixture at
$N_C/N_U = 600$ is just critical ($k_\infty = 1$). You need a `Mix` with your
own U-235 fraction (copy `natural_mix` in `vv::ugraphite`) and a bisection on
it. How would you put an error bar on that enrichment?

## The whole call tree

Everything `ugraphite_four_factor.rs`'s `main` reaches inside the workspace,
three calls deep, as an architecture map. Generated by `kovan-cli code-walk`
(std and dependency calls left out; each function expanded once).

<div class="codewalk">

<!-- code-walk: from=crates/outram-mc-libs/examples/ugraphite_four_factor.rs::main depth=3 -->
<!-- generated by `kovan-cli code-walk-check --update`; edit the comment above, not this -->

Everything `crates/outram-mc-libs/examples/ugraphite_four_factor.rs::main` reaches in the workspace, to 3 hops: 47 functions, 8 unresolved calls. A function is expanded once; later calls to it say *(expanded elsewhere in this walk)*.

<div class="cw-node" style="margin-left:0.0em">

[`ugraphite_four_factor.rs::main`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L300) `fn main()`

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:300 fn main -->
<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:302 env_or -->
<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:305 load_nuclides -->
<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:317 sweep -->
<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:318 main_case -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/examples/ugraphite_four_factor.rs:300:319}}
    // … (the rest of the function: follow the link above)
```

</details>
</div>

<div class="cw-node" style="margin-left:0.9em">

[`ugraphite_common.rs::env_or`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L75) `pub fn env_or<T: std::str::FromStr>(k: &str, d: T) -> T` · called at [L302](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L302)

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/examples/common/ugraphite_common.rs:75 fn env_or -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/examples/common/ugraphite_common.rs:75:80}}
```

</details>
</div>

<div class="cw-node" style="margin-left:0.9em">

[`ugraphite_common.rs::load_nuclides`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L89) `pub fn load_nuclides() -> Vec<Nuclide>` — U-234, U-235, U-238, C-12, C-13 from ENDF/B-VIII.0 at `TEMP_K` (RECONR + BROADR, tolerance 0.001; URR and DBRC by the constructor's defaults), with crystalline-graphite S(alpha,beta) (MAT 30, 296 K) on both carbons. · called at [L305](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L305)

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/examples/common/ugraphite_common.rs:89 fn load_nuclides -->
<!-- snippet-check: crates/outram-mc-libs/examples/common/ugraphite_common.rs:96 reference_endf -->
<!-- snippet-check: crates/outram-mc-libs/examples/common/ugraphite_common.rs:98 from_endf_file -->
<!-- snippet-check: crates/outram-mc-libs/examples/common/ugraphite_common.rs:104 from_endf_file -->
<!-- snippet-check: crates/outram-mc-libs/examples/common/ugraphite_common.rs:109 load -->
<!-- snippet-check: crates/outram-mc-libs/examples/common/ugraphite_common.rs:111 with_thermal_scattering -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/examples/common/ugraphite_common.rs:89:112}}
    // … (the rest of the function: follow the link above)
```

</details>
</div>

<div class="cw-node" style="margin-left:1.8em">

[`reference_data.rs::reference_endf`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/reference_data.rs#L91) `pub fn reference_endf(file: &str) -> Option<PathBuf>` — Absolute path of reference tape `file` (e.g. · called at [L96](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L96)

<details><summary>code</summary>

<!-- snippet-check: crates/njoy-outram-park-fork/src/reference_data.rs:91 fn reference_endf -->
<!-- snippet-check: crates/njoy-outram-park-fork/src/reference_data.rs:94 reference_endf_dir -->

```rust,ignore
{{#include ../../../../../crates/njoy-outram-park-fork/src/reference_data.rs:91:95}}
    // … (the rest of the function: follow the link above)
```

</details>
</div>

<div class="cw-node" style="margin-left:2.7em">

[`reference_data.rs::reference_endf_dir`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/reference_data.rs#L52) `pub fn reference_endf_dir() -> PathBuf` — The directory reference tapes are read from, whether or not it exists. · called at [L94](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/reference_data.rs#L94) · *(calls below the depth limit not shown)*

<details><summary>code</summary>

<!-- snippet-check: crates/njoy-outram-park-fork/src/reference_data.rs:52 fn reference_endf_dir -->

```rust,ignore
{{#include ../../../../../crates/njoy-outram-park-fork/src/reference_data.rs:52:61}}
```

</details>
</div>

<div class="cw-node" style="margin-left:1.8em">

[`nuclide.rs::Nuclide::from_endf_file`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L1642) `pub fn from_endf_file(path: &std::path::Path, name: &str, temp_k: f64, tolerance: f64) -> Result<Self, NjoyError>` — Build a nuclide from an ENDF file **on disk** — the ordinary case. · called at [L98](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L98)

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/src/material/nuclide.rs:1642 fn from_endf_file -->
<!-- snippet-check: crates/outram-mc-libs/src/material/nuclide.rs:1648 read_file -->
<!-- snippet-check: crates/outram-mc-libs/src/material/nuclide.rs:1652 materials -->
<!-- snippet-check: crates/outram-mc-libs/src/material/nuclide.rs:1655 from_tape -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/material/nuclide.rs:1642:1656}}
```

</details>
</div>

<div class="cw-node" style="margin-left:2.7em">

[`tape.rs::Tape::read_file`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/endf/tape.rs#L98) `pub fn read_file(path: &std::path::Path) -> Result<Self, NjoyError>` — Parse an ENDF ASCII tape from a file on disk. · called at [L1648](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L1648) · *(calls below the depth limit not shown)*

<details><summary>code</summary>

<!-- snippet-check: crates/njoy-outram-park-fork/src/endf/tape.rs:98 fn read_file -->

```rust,ignore
{{#include ../../../../../crates/njoy-outram-park-fork/src/endf/tape.rs:98:105}}
```

</details>
</div>

<div class="cw-node" style="margin-left:2.7em">

[`tape.rs::Tape::materials`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/endf/tape.rs#L225) `pub fn materials(&self) -> Vec<i32>` — Every ENDF material number on this tape, ascending and deduplicated. · called at [L1652](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L1652) · *(calls below the depth limit not shown)*

<details><summary>code</summary>

<!-- snippet-check: crates/njoy-outram-park-fork/src/endf/tape.rs:225 fn materials -->

```rust,ignore
{{#include ../../../../../crates/njoy-outram-park-fork/src/endf/tape.rs:225:230}}
```

</details>
</div>

<div class="cw-node" style="margin-left:2.7em">

[`nuclide.rs::Nuclide::from_tape`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L2360) `pub fn from_tape(tape: &njoy_outram_park_fork::endf::tape::Tape, mat: i32, name: &str, temp_k: f64, tolerance: f64) -> Result<Self, NjoyError>` — Build a nuclide from an ENDF tape **already in hand** — no network, no feature gate. · called at [L1655](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L1655) · *(calls below the depth limit not shown)*

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/src/material/nuclide.rs:2360 fn from_tape -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/material/nuclide.rs:2360:2370}}
```

</details>
</div>

<div class="cw-node" style="margin-left:1.8em">

[`thermal.rs::ThermalScattering::from_endf_file`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/thermal.rs#L639) `pub fn from_endf_file(path: &str, mat: i32, temperature_k: f64, name: &str) -> Result<Self, NjoyError>` — Build the pre-tabulated bound-atom thermal treatment — inelastic **and** elastic — from an ENDF `tsl-*` thermal evaluation file. · called at [L104](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L104)

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/src/material/thermal.rs:639 fn from_endf_file -->
<!-- snippet-check: crates/outram-mc-libs/src/material/thermal.rs:647 read -->
<!-- snippet-check: crates/outram-mc-libs/src/material/thermal.rs:648 from_tape -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/material/thermal.rs:639:649}}
```

</details>
</div>

<div class="cw-node" style="margin-left:2.7em">

[`tape.rs::Tape::read`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/endf/tape.rs#L113) `pub fn read<R: Read>(reader: R) -> Result<Self, NjoyError>` — Parse an ENDF ASCII tape from any `Read` source. · called at [L647](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/thermal.rs#L647) · *(calls below the depth limit not shown)*

<details><summary>code</summary>

<!-- snippet-check: crates/njoy-outram-park-fork/src/endf/tape.rs:113 fn read -->

```rust,ignore
{{#include ../../../../../crates/njoy-outram-park-fork/src/endf/tape.rs:113:152}}
    // … (the rest of the function: follow the link above)
```

</details>
</div>

<div class="cw-node" style="margin-left:2.7em">

[`thermal.rs::ThermalScattering::from_tape`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/thermal.rs#L818) `pub fn from_tape(tape: &njoy_outram_park_fork::endf::tape::Tape, mat: i32, temperature_k: f64, name: &str) -> Result<Self, NjoyError>` · called at [L648](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/thermal.rs#L648) · *(calls below the depth limit not shown)*

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/src/material/thermal.rs:818 fn from_tape -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/material/thermal.rs:818:825}}
```

</details>
</div>

<div class="cw-node" style="margin-left:1.8em">

UNRESOLVED(closure): `load` at [L109](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L109) (→ [`crates/outram-mc-libs/examples/common/ugraphite_common.rs:95`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L95)) — call through a closure or fn-typed binding `load`

</div>

<div class="cw-node" style="margin-left:1.8em">

[`nuclide.rs::Nuclide::with_thermal_scattering`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L605) `pub fn with_thermal_scattering(mut self, thermal: ThermalScattering) -> Self` — Attach a bound-atom S(α,β) `ThermalScattering` treatment to this nuclide (builder style, consumes and returns `self`). · called at [L111](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L111)

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/src/material/nuclide.rs:605 fn with_thermal_scattering -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/material/nuclide.rs:605:608}}
```

</details>
</div>

<div class="cw-node" style="margin-left:1.8em">

UNRESOLVED(other): `clone` at [L111](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L111) (→ [`crates/outram-mc-libs/src/material/thermal.rs:542`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/thermal.rs#L542)) — resolves to `#[derive(Debug, Clone)]`, not a function body

</div>

<div class="cw-node" style="margin-left:0.9em">

[`ugraphite_four_factor.rs::sweep`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L378) `fn sweep(nuclides: &[Nuclide], seed: u64)` · called at [L317](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L317)

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:378 fn sweep -->
<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:389 natural_mix -->
<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:391 run_homogeneous -->
<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:392 print_case -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/examples/ugraphite_four_factor.rs:378:393}}
    // … (the rest of the function: follow the link above)
```

</details>
</div>

<div class="cw-node" style="margin-left:1.8em">

[`ugraphite.rs::natural_mix`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/vv/ugraphite.rs#L181) `pub fn natural_mix(c_per_u: f64) -> Mix` — Natural uranium in graphite at 1.73 g/cm3 carbon, `c_per_u` carbon atoms per uranium atom. · called at [L389](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L389)

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/src/vv/ugraphite.rs:181 fn natural_mix -->
<!-- snippet-check: crates/outram-mc-libs/src/vv/ugraphite.rs:182 graphite_density -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/vv/ugraphite.rs:181:183}}
    // … (the rest of the function: follow the link above)
```

</details>
</div>

<div class="cw-node" style="margin-left:2.7em">

[`ugraphite.rs::graphite_density`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/vv/ugraphite.rs#L174) `pub fn graphite_density() -> f64` — Graphite at 1.73 g/cm3 (`RHO_GRAPHITE`) \[atoms/b-cm\]. · called at [L182](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/vv/ugraphite.rs#L182) · *(calls below the depth limit not shown)*

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/src/vv/ugraphite.rs:174 fn graphite_density -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/vv/ugraphite.rs:174:176}}
```

</details>
</div>

<div class="cw-node" style="margin-left:1.8em">

[`ugraphite_four_factor.rs::run_homogeneous`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L293) `fn run_homogeneous(label: &str, mix: Mix, nuclides: &[Nuclide], seed: u64) -> CaseResult` — The homogeneous medium: one material filling the reflective cube. · called at [L391](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L391)

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:293 fn run_homogeneous -->
<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:294 homogeneous_cube -->
<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:295 material -->
<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:296 run_case -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/examples/ugraphite_four_factor.rs:293:297}}
```

</details>
</div>

<div class="cw-node" style="margin-left:2.7em">

[`fhr_pebble.rs::homogeneous_cube`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/pebble_beds/fhr_pebble.rs#L538) `pub fn homogeneous_cube(h: f64, material_idx: usize, temperature: f64) -> Geometry` — A homogeneous-medium cube geometry of half-width `h` \[cm\] filled with a single material, reflective on all six faces — the k∞ unit cell for comparing an explicit packing against its homogenised equivalent. · called at [L294](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L294) · *(calls below the depth limit not shown)*

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/src/pebble_beds/fhr_pebble.rs:538 fn homogeneous_cube -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/pebble_beds/fhr_pebble.rs:538:577}}
    // … (the rest of the function: follow the link above)
```

</details>
</div>

<div class="cw-node" style="margin-left:2.7em">

[`ugraphite.rs::Mix::material`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/vv/ugraphite.rs#L97) `pub fn material(&self, id: i32, name: &str) -> Material` — The material, with nuclide indices in `TAPES` order and zero densities dropped, at `TEMPERATURE_K`. · called at [L295](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L295) · *(calls below the depth limit not shown)*

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/src/vv/ugraphite.rs:97 fn material -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/vv/ugraphite.rs:97:113}}
```

</details>
</div>

<div class="cw-node" style="margin-left:2.7em">

[`ugraphite_common.rs::run_case`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L170) `pub fn run_case(geom: &Geometry, mats: &[Material], nuclides: &[Nuclide], seed: u64, source_half_cm: f64, split: FuelSplit) -> CaseResult` — Run the power iteration with six-factor capture on `geom` and assemble the factors with the requested fuel definition. · called at [L296](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L296) · *(calls below the depth limit not shown)*

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/examples/common/ugraphite_common.rs:170 fn run_case -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/examples/common/ugraphite_common.rs:170:209}}
    // … (the rest of the function: follow the link above)
```

</details>
</div>

<div class="cw-node" style="margin-left:1.8em">

[`ugraphite_common.rs::print_case`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L244) `pub fn print_case(label: &str, c_per_u: f64, r: &CaseResult)` — Print one case: k, the three-group factors, telescoping, the two-group view, the group rates and the fuel split. · called at [L392](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L392)

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/examples/common/ugraphite_common.rs:244 fn print_case -->
<!-- snippet-check: crates/outram-mc-libs/examples/common/ugraphite_common.rs:255 e -->
<!-- snippet-check: crates/outram-mc-libs/examples/common/ugraphite_common.rs:256 e -->
<!-- snippet-check: crates/outram-mc-libs/examples/common/ugraphite_common.rs:257 e -->
<!-- snippet-check: crates/outram-mc-libs/examples/common/ugraphite_common.rs:258 e -->
<!-- snippet-check: crates/outram-mc-libs/examples/common/ugraphite_common.rs:259 e -->
<!-- snippet-check: crates/outram-mc-libs/examples/common/ugraphite_common.rs:260 e -->
<!-- snippet-check: crates/outram-mc-libs/examples/common/ugraphite_common.rs:281 two_group_openmc_convention -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/examples/common/ugraphite_common.rs:244:244}}
    // …
{{#include ../../../../../crates/outram-mc-libs/examples/common/ugraphite_common.rs:253:261}}
    // …
{{#include ../../../../../crates/outram-mc-libs/examples/common/ugraphite_common.rs:279:282}}
    // … (the rest of the function: follow the link above)
```

</details>
</div>

<div class="cw-node" style="margin-left:2.7em">

UNRESOLVED(closure): `e` at [L255](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L255) (→ [`crates/outram-mc-libs/examples/common/ugraphite_common.rs:247`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L247)) — call through a closure or fn-typed binding `e`

</div>

<div class="cw-node" style="margin-left:2.7em">

UNRESOLVED(closure): `e` at [L256](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L256) (→ [`crates/outram-mc-libs/examples/common/ugraphite_common.rs:247`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L247)) — call through a closure or fn-typed binding `e`

</div>

<div class="cw-node" style="margin-left:2.7em">

UNRESOLVED(closure): `e` at [L257](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L257) (→ [`crates/outram-mc-libs/examples/common/ugraphite_common.rs:247`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L247)) — call through a closure or fn-typed binding `e`

</div>

<div class="cw-node" style="margin-left:2.7em">

UNRESOLVED(closure): `e` at [L258](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L258) (→ [`crates/outram-mc-libs/examples/common/ugraphite_common.rs:247`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L247)) — call through a closure or fn-typed binding `e`

</div>

<div class="cw-node" style="margin-left:2.7em">

UNRESOLVED(closure): `e` at [L259](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L259) (→ [`crates/outram-mc-libs/examples/common/ugraphite_common.rs:247`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L247)) — call through a closure or fn-typed binding `e`

</div>

<div class="cw-node" style="margin-left:2.7em">

UNRESOLVED(closure): `e` at [L260](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L260) (→ [`crates/outram-mc-libs/examples/common/ugraphite_common.rs:247`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L247)) — call through a closure or fn-typed binding `e`

</div>

<div class="cw-node" style="margin-left:2.7em">

[`reactor_physics.rs::SixFactors::two_group_openmc_convention`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/reactor_physics.rs#L291) `pub fn two_group_openmc_convention(&self) -> (f64, f64, f64, f64)` — The same run re-expressed in the **two-group** convention the OpenMC reference deck uses, for like-for-like comparison. · called at [L281](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L281) · *(calls below the depth limit not shown)*

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/src/physics/reactor_physics.rs:291 fn two_group_openmc_convention -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/physics/reactor_physics.rs:291:329}}
```

</details>
</div>

<div class="cw-node" style="margin-left:1.8em">

[`ugraphite.rs::Mix::c_per_u`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/vv/ugraphite.rs#L80) `pub fn c_per_u(&self) -> f64` — Carbon atoms per uranium atom. · called at [L392](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L392)

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/src/vv/ugraphite.rs:80 fn c_per_u -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/vv/ugraphite.rs:80:82}}
```

</details>
</div>

<div class="cw-node" style="margin-left:0.9em">

[`ugraphite_four_factor.rs::main_case`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L322) `fn main_case(nuclides: &[Nuclide], seed: u64)` · called at [L318](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L318)

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:322 fn main_case -->
<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:323 htr10_pebble_mix -->
<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:331 run_homogeneous -->
<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:332 print_case -->
<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:339 env_or -->
<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:342 slow_down -->
<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:344 densities -->
<!-- snippet-check: crates/outram-mc-libs/examples/ugraphite_four_factor.rs:350 default -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/examples/ugraphite_four_factor.rs:322:351}}
    // … (the rest of the function: follow the link above)
```

</details>
</div>

<div class="cw-node" style="margin-left:1.8em">

[`ugraphite_common.rs::htr10_pebble_mix`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L66) `pub fn htr10_pebble_mix() -> Mix` — One HTR-10 fuel pebble's uranium and carbon, smeared over the ball (`vv::ugraphite::htr10_pebble_mix`), with its inventory printed. · called at [L323](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L323)

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/examples/common/ugraphite_common.rs:66 fn htr10_pebble_mix -->
<!-- snippet-check: crates/outram-mc-libs/examples/common/ugraphite_common.rs:67 htr10_pebble_mix -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/examples/common/ugraphite_common.rs:66:68}}
    // … (the rest of the function: follow the link above)
```

</details>
</div>

<div class="cw-node" style="margin-left:2.7em">

[`ugraphite.rs::htr10_pebble_mix`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/vv/ugraphite.rs#L139) `pub fn htr10_pebble_mix() -> (Mix, PebbleInventory)` — One HTR-10 fuel pebble's uranium and carbon, smeared over the 3 cm ball, and the inventory it came from. · called at [L67](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/ugraphite_common.rs#L67) · *(calls below the depth limit not shown)*

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/src/vv/ugraphite.rs:139 fn htr10_pebble_mix -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/vv/ugraphite.rs:139:171}}
```

</details>
</div>

<div class="cw-node" style="margin-left:1.8em">

`ugraphite_four_factor.rs::run_homogeneous` *(expanded elsewhere in this walk)* · called at [L331](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L331)

</div>

<div class="cw-node" style="margin-left:1.8em">

`ugraphite_common.rs::print_case` *(expanded elsewhere in this walk)* · called at [L332](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L332)

</div>

<div class="cw-node" style="margin-left:1.8em">

`ugraphite.rs::Mix::c_per_u` *(expanded elsewhere in this walk)* · called at [L332](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L332)

</div>

<div class="cw-node" style="margin-left:1.8em">

`ugraphite_common.rs::env_or` *(expanded elsewhere in this walk)* · called at [L339](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L339)

</div>

<div class="cw-node" style="margin-left:1.8em">

[`energy_only_slowing_down.rs::slow_down`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs#L119) `pub fn slow_down(nuclides: &[Nuclide], densities: &[f64], temp_k: f64, e_source: f64, e_cut: f64, histories: usize, seed: &mut u64, opts: KernelOptions) -> SlowDownCounts` — Follow `histories` neutrons in **energy only** from `e_source` down to `e_cut`, through the mixture `nuclides[i]` at `densities[i]` \[atoms/barn·cm\], and count where they end up. · called at [L342](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L342)

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs:119 fn slow_down -->
<!-- snippet-check: crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs:131 new -->
<!-- snippet-check: crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs:148 future_seed -->
<!-- snippet-check: crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs:154 needs_urr_draw -->
<!-- snippet-check: crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs:155 xs_at_energy_urr -->
<!-- snippet-check: crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs:157 xs_at_energy -->
<!-- snippet-check: crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs:166 prn -->
<!-- snippet-check: crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs:184 sample_inelastic -->
<!-- snippet-check: crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs:185 sample_inelastic_mu_cm -->
<!-- snippet-check: crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs:188 two_body_scatter_with_mu -->
<!-- snippet-check: crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs:190 two_body_scatter -->
<!-- snippet-check: crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs:193 continuum_inelastic_scatter_evaluated -->
<!-- snippet-check: crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs:198 continuum_law -->
<!-- snippet-check: crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs:221 sample_thermal -->
<!-- snippet-check: crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs:228 sample_elastic_mu_cm -->
<!-- snippet-check: crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs:231 free_gas_kt -->
<!-- snippet-check: crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs:235 dbrc_table -->
<!-- snippet-check: crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs:236 free_gas_elastic_scatter_dbrc -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs:119:126}}
    // …
{{#include ../../../../../crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs:129:132}}
    // …
{{#include ../../../../../crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs:146:149}}
    // …
{{#include ../../../../../crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs:152:158}}
    // …
{{#include ../../../../../crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs:164:167}}
    // …
{{#include ../../../../../crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs:182:194}}
    // …
{{#include ../../../../../crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs:196:199}}
    // …
{{#include ../../../../../crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs:219:222}}
    // …
{{#include ../../../../../crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs:226:237}}
    // … (the rest of the function: follow the link above)
```

</details>
</div>

<div class="cw-node" style="margin-left:2.7em">

[`position.rs::Direction::new`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-blender/src/csg/position.rs#L93) `pub fn new(u: f64, v: f64, w: f64) -> Self` — Construct a Direction from raw components — caller must ensure |d| ≈ 1. · called at [L131](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs#L131) · *(calls below the depth limit not shown)*

<details><summary>code</summary>

<!-- snippet-check: crates/outram-blender/src/csg/position.rs:93 fn new -->

```rust,ignore
{{#include ../../../../../crates/outram-blender/src/csg/position.rs:93:95}}
```

</details>
</div>

<div class="cw-node" style="margin-left:2.7em">

[`lcg.rs::future_seed`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/petir/src/rng/lcg.rs#L145) `pub fn future_seed(mut n: u64, seed: u64) -> u64` — Advance the seed `n` steps in O(log n) using the LCG jump-ahead identity. · called at [L148](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs#L148) · *(calls below the depth limit not shown)*

<details><summary>code</summary>

<!-- snippet-check: crates/petir/src/rng/lcg.rs:145 fn future_seed -->

```rust,ignore
{{#include ../../../../../crates/petir/src/rng/lcg.rs:145:160}}
```

</details>
</div>

<div class="cw-node" style="margin-left:2.7em">

[`nuclide.rs::Nuclide::needs_urr_draw`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L1210) `pub fn needs_urr_draw(&self, e: f64) -> bool` — Whether a collision on this nuclide at energy `e` \[eV\] requires a URR band draw. · called at [L154](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs#L154) · *(calls below the depth limit not shown)*

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/src/material/nuclide.rs:1210 fn needs_urr_draw -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/material/nuclide.rs:1210:1212}}
```

</details>
</div>

<div class="cw-node" style="margin-left:2.7em">

[`nuclide.rs::Nuclide::xs_at_energy_urr`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L1530) `pub fn xs_at_energy_urr(&self, e: f64, temp_k: f64, xi: f64) -> MicroXS` — This nuclide's microscopic cross sections at `e` \[eV\] and `temp_k` \[K\], **with unresolved-resonance self-shielding applied** from the sampled band `xi`. · called at [L155](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs#L155) · *(calls below the depth limit not shown)*

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/src/material/nuclide.rs:1530 fn xs_at_energy_urr -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/material/nuclide.rs:1530:1560}}
```

</details>
</div>

<div class="cw-node" style="margin-left:2.7em">

[`material.rs::urr_xi`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/material.rs#L252) `pub fn urr_xi(nuclide_idx: usize, urr_seed: u64) -> f64` — The URR band variate of nuclide `nuclide_idx` (its index in the global nuclide array) for a neutron whose URR stream seed is `urr_seed`: OpenMC's `future_prn(index_, p.seeds(STREAM_URR_PTABLE))` (`src/nuclide.cpp`, `calculate_urr_xs`). · called at [L155](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs#L155) · *(calls below the depth limit not shown)*

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/src/material/material.rs:252 fn urr_xi -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/material/material.rs:252:255}}
```

</details>
</div>

<div class="cw-node" style="margin-left:2.7em">

[`nuclide.rs::Nuclide::xs_at_energy`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L3106) `pub fn xs_at_energy(&self, e: f64, temp_k: f64) -> MicroXS` · called at [L157](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs#L157) · *(calls below the depth limit not shown)*

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/src/material/nuclide.rs:3106 fn xs_at_energy -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/material/nuclide.rs:3106:3130}}
```

</details>
</div>

<div class="cw-node" style="margin-left:2.7em">

[`lcg.rs::prn`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/petir/src/rng/lcg.rs#L127) `pub fn prn(seed: &mut u64) -> f64` — Advance the seed one step and return a uniform sample in [0, 1). · called at [L166](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs#L166) · *(calls below the depth limit not shown)*

<details><summary>code</summary>

<!-- snippet-check: crates/petir/src/rng/lcg.rs:127 fn prn -->

```rust,ignore
{{#include ../../../../../crates/petir/src/rng/lcg.rs:127:138}}
```

</details>
</div>

<div class="cw-node" style="margin-left:2.7em">

[`nuclide.rs::Nuclide::sample_inelastic`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L3349) `pub fn sample_inelastic(&self, e: f64, seed: &mut u64) -> Inelastic` — Sample which inelastic scattering channel a collision at energy `e` \[eV\] takes, proportional to each channel's cross section at `e`. · called at [L184](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs#L184) · *(calls below the depth limit not shown)*

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/src/material/nuclide.rs:3349 fn sample_inelastic -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/material/nuclide.rs:3349:3383}}
```

</details>
</div>

<div class="cw-node" style="margin-left:2.7em">

[`nuclide.rs::Nuclide::sample_inelastic_mu_cm`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L3449) `pub fn sample_inelastic_mu_cm(&self, mt: i32, e: f64, seed: &mut u64) -> Option<f64>` — Sample a **discrete inelastic** (MT=51…90) scattering cosine in the **centre-of-mass frame** at incident energy `e` \[eV\], returning `Some(mu_cm)` when this nuclide carries an MF=4 distribution for that level, or `None` when it does not — the caller then falls back to isotropic-CM, which is what this crate did for *every* inelastic collision before bead `op-tm9f`. · called at [L185](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs#L185) · *(calls below the depth limit not shown)*

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/src/material/nuclide.rs:3449 fn sample_inelastic_mu_cm -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/material/nuclide.rs:3449:3461}}
```

</details>
</div>

<div class="cw-node" style="margin-left:2.7em">

[`scatter.rs::two_body_scatter_with_mu`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/scatter.rs#L178) `pub fn two_body_scatter_with_mu(e: f64, u: Direction, awr: f64, q: f64, mu_cm: f64, seed: &mut u64) -> (f64, Direction)` — Two-body scatter with a **caller-supplied** centre-of-mass scattering cosine `mu_cm` — the anisotropic form of `two_body_scatter`. · called at [L188](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs#L188) · *(calls below the depth limit not shown)*

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/src/physics/scatter.rs:178 fn two_body_scatter_with_mu -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/physics/scatter.rs:178:191}}
```

</details>
</div>

<div class="cw-node" style="margin-left:2.7em">

[`scatter.rs::two_body_scatter`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/scatter.rs#L157) `pub fn two_body_scatter(e: f64, u: Direction, awr: f64, q: f64, seed: &mut u64) -> (f64, Direction)` — Two-body scatter a neutron of energy `e` \[eV\] and direction `u` off a target of atomic weight ratio `awr` with reaction Q-value `q` \[eV\], isotropic in the centre-of-mass frame. · called at [L190](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs#L190) · *(calls below the depth limit not shown)*

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/src/physics/scatter.rs:157 fn two_body_scatter -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/physics/scatter.rs:157:166}}
```

</details>
</div>

<div class="cw-node" style="margin-left:2.7em">

[`scatter.rs::continuum_inelastic_scatter_evaluated`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/scatter.rs#L759) `pub fn continuum_inelastic_scatter_evaluated(e: f64, u: Direction, awr: f64, q: f64, law: Option<&ContinuumEmission>, seed: &mut u64) -> (f64, Direction)` — Continuum inelastic (MT=91) or (n,2n) (MT=16) scatter using the **evaluated** ENDF MF=6 LAW=1 emission law when the nuclide carries one, falling back to `continuum_inelastic_scatter`'s Weisskopf evaporation stand-in when it does not. · called at [L193](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs#L193) · *(calls below the depth limit not shown)*

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/src/physics/scatter.rs:759 fn continuum_inelastic_scatter_evaluated -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/physics/scatter.rs:759:776}}
```

</details>
</div>

<div class="cw-node" style="margin-left:2.7em">

[`nuclide.rs::Nuclide::continuum_law`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L2769) `pub fn continuum_law(&self, mt: i32) -> Option<&ContinuumEmission>` — Microscopic cross sections at incident energy `e` \[eV\] and temperature `temp_k` \[K\]. · called at [L198](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs#L198) · *(calls below the depth limit not shown)*

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/src/material/nuclide.rs:2769 fn continuum_law -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/material/nuclide.rs:2769:2777}}
```

</details>
</div>

<div class="cw-node" style="margin-left:2.7em">

[`nuclide.rs::Nuclide::sample_thermal`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L3480) `pub fn sample_thermal(&self, e: f64, seed: &mut u64) -> Option<(f64, f64)>` — Sample a bound-atom S(α,β) thermal scatter at incident energy `e` \[eV\], returning `Some((e_out, mu_lab))` — a **laboratory-frame** outgoing energy \[eV\] and scattering cosine — when this nuclide carries a `ThermalScattering` table and `e` is below its cutoff, or `None` otherwise (the caller then falls back to free-gas / anisotropic elastic). · called at [L221](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs#L221) · *(calls below the depth limit not shown)*

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/src/material/nuclide.rs:3480 fn sample_thermal -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/material/nuclide.rs:3480:3482}}
```

</details>
</div>

<div class="cw-node" style="margin-left:2.7em">

[`nuclide.rs::Nuclide::sample_elastic_mu_cm`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L3405) `pub fn sample_elastic_mu_cm(&self, e: f64, seed: &mut u64) -> Option<f64>` — Sample an elastic scattering cosine in the **centre-of-mass frame** at incident energy `e` \[eV\], returning `Some(mu_cm)` for anisotropic elastic or `None` when the distribution is isotropic (the caller then falls back to isotropic-CM elastic). · called at [L228](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs#L228) · *(calls below the depth limit not shown)*

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/src/material/nuclide.rs:3405 fn sample_elastic_mu_cm -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/material/nuclide.rs:3405:3423}}
```

</details>
</div>

<div class="cw-node" style="margin-left:2.7em">

[`nuclide.rs::Nuclide::free_gas_kt`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L854) `pub fn free_gas_kt(&self, lookup_temp_k: f64) -> f64` — The `k_B·T` \[eV\] this nuclide's **elastic kinematics** (free gas and DBRC) use in a collision whose cross sections are looked up at `lookup_temp_k` \[K\]. · called at [L231](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs#L231) · *(calls below the depth limit not shown)*

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/src/material/nuclide.rs:854 fn free_gas_kt -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/material/nuclide.rs:854:862}}
```

</details>
</div>

<div class="cw-node" style="margin-left:2.7em">

[`nuclide.rs::Nuclide::dbrc_table`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L1493) `pub fn dbrc_table(&self) -> Option<&DbrcTable>` — This nuclide's DBRC table, for the transport kernels to hand to `free_gas_elastic_scatter_dbrc`. · called at [L235](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs#L235) · *(calls below the depth limit not shown)*

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/src/material/nuclide.rs:1493 fn dbrc_table -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/material/nuclide.rs:1493:1495}}
```

</details>
</div>

<div class="cw-node" style="margin-left:2.7em">

[`scatter.rs::free_gas_elastic_scatter_dbrc`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/scatter.rs#L265) `pub fn free_gas_elastic_scatter_dbrc(e: f64, u: Direction, awr: f64, kt_ev: f64, mu_cm: f64, seed: &mut u64, dbrc: Option<&DbrcTable>) -> (f64, Direction)` — `free_gas_elastic_scatter` with an optional **DBRC** correction. · called at [L236](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs#L236) · *(calls below the depth limit not shown)*

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/src/physics/scatter.rs:265 fn free_gas_elastic_scatter_dbrc -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/physics/scatter.rs:265:304}}
    // … (the rest of the function: follow the link above)
```

</details>
</div>

<div class="cw-node" style="margin-left:1.8em">

[`ugraphite.rs::Mix::densities`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/vv/ugraphite.rs#L85) `pub fn densities(&self) -> [f64; 5]` — Atom densities in `TAPES` order: U-234, U-235, U-238, C-12, C-13. · called at [L344](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L344)

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/src/vv/ugraphite.rs:85 fn densities -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/src/vv/ugraphite.rs:85:93}}
```

</details>
</div>

<div class="cw-node" style="margin-left:1.8em">

[`energy_only_slowing_down.rs::KernelOptions::default`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs#L77) `fn default() -> Self` · called at [L350](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/ugraphite_four_factor.rs#L350)

<details><summary>code</summary>

<!-- snippet-check: crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs:77 fn default -->

```rust,ignore
{{#include ../../../../../crates/outram-mc-libs/examples/common/energy_only_slowing_down.rs:77:83}}
```

</details>
</div>
<!-- /code-walk -->

</div>

In the demo, the browser's worker reaches the same transport through
[`ugraphite::sim::Chain::run_next`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/dhoby-ghaut/examples/monte_carlo_web/ugraphite/sim.rs#@@L:crates/dhoby-ghaut/examples/monte_carlo_web/ugraphite/sim.rs:fn=run_next@@)
(`run_fixed_source_traced`, one neutron per call), called from
[`Loaded::serve`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/dhoby-ghaut/examples/monte_carlo_web/engine.rs#@@L:crates/dhoby-ghaut/examples/monte_carlo_web/engine.rs:fn=serve@@)
when the page asks for the next neutron *(filled by hand: page and worker
talk by messages, not calls)*.

---

**Deliberate liberties.** The main case is one HTR-10 pebble's uranium and
carbon only: the oxygen of the UO₂, the silicon of the SiC and the boron
impurities are left out (this rung is uranium and graphite, by design), the
gaps between pebbles are not modelled, and the TRISO particles are smeared,
which is exactly the heterogeneity rungs 3 and 5 put back. None of these is
measured here. Natural uranium is the IUPAC composition, recalled and not
page-checked (`vv::ugraphite::NAT_U`). Everything is at 296 K, the lowest
tabulated temperature of the graphite law. The demo's Watch mode processes
the data at the loosened tolerance 0.01 (NJOY's is 0.001).

**Literature.** ENDF/B-VIII.0 [(Brown & others, 2018)](#ref-brown2018endf8)
for every cross section and the graphite thermal scattering law; the HTR-10
pebble specification as cited in `pebble_beds::htr10` ([Li et al., 2014](#ref-li2014htr10rmc),
Table 2; IAEA-TECDOC-1382 [International Atomic Energy Agency, 2003](#ref-iaeatecdoc1382), Table 4-2, private corpus, cited by page only).

**Doesn't tally?** If anything here disagrees with the code it links to, the
page is wrong:
[report it](https://github.com/theodoreOnzGit/outram-park-backend/issues/new?title=Uranium-in-graphite%20lesson%20doesn%27t%20tally%3A%20&labels=bug).
This page changes whenever `develop` does; it was built from
`@@COMMIT_SHORT@@` on @@BUILD_DATE@@. Tracking issue
[#524](https://github.com/theodoreOnzGit/outram-park-backend/issues/524).

<!-- references:begin -->
## References

<p class="csl-entry" id="ref-brown2018endf8" style="padding-left: 2em; text-indent: -2em;">Brown, D. A. &#38; others. (2018). ENDF/B-VIII.0: The 8th Major Release of the Nuclear Reaction Data Library with CIELO-project Cross Sections, New Standards and Thermal Scattering Data. <i>Nuclear Data Sheets</i>, <i>148</i>, 1–142. https://doi.org/10.1016/j.nds.2018.02.001</p>

<p class="csl-entry" id="ref-iaeatecdoc1382" style="padding-left: 2em; text-indent: -2em;">International Atomic Energy Agency. (2003). <i>Evaluation of High Temperature Gas Cooled Reactor Performance</i> (IAEA-TECDOC-1382). International Atomic Energy Agency.</p>

<p class="csl-entry" id="ref-li2014htr10rmc" style="padding-left: 2em; text-indent: -2em;">Li, W., Yu, G., &#38; Wei, C. (2014, October). Research on Benchmark Calculation and Analysis of HTR-10 with RMC Code. <i>7th International Topical Meeting on High Temperature Reactor Technology (HTR 2014)</i>.</p>

<!-- references:end -->
