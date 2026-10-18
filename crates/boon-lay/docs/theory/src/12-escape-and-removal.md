# Step 12 — Escape and removal: purification, leak, plate-out and lift-off

> **Research, education and V&V only.** Not for reactor operation, licensing, safety decisions or emergency response.
> **Review status: first draft, 2026-10-18, AI-assisted (Claude Sonnet), not yet reviewed by a human.**

**The question this page answers.** [Step 11](./11-into-the-coolant.md) put
fission products into the coolant. *By what routes do they leave it, how do
different chemistries take different routes, and where does what stays behind
end up?*

## 1. Different chemistry, different fate

This follows [Step 3](./03-layers-chemistry.md): chemistry decides how an atom
behaves, and the coolant circuit is no exception.

- **Noble gases (Kr, Xe)** are inert. They do not stick to surfaces. They
  stay in the helium until they decay, are taken out by the purification
  system, or leak out with the helium.
- **Halogens (I, and others)** are reactive. Part sticks to cold surfaces and
  part stays in the gas. The code treats them as both plating out *and*
  purified.
- **Metals (Cs, Sr, Ag, Ba, Eu and others)** are not volatile in the gas at
  circuit temperatures. They deposit on surfaces. This is *plate-out*.

TRISO-ATOPS encodes exactly that split. It routes each nuclide's two removal
rates by group
([`removal_rates.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/triso_atops_extensions/removal_rates.rs)):

| Group | Plate-out $k_{plate}$ | Purification $k_{clean}$ |
|---|---|---|
| noble gases | 0 | $k_{clean}$ |
| halogens | $k_{plate}$ | $k_{clean}$ |
| special metals, silver, other | $k_{plate}$ | 0 |

This table is upstream's routing, kept. The reasons for each cell are
chemical judgements the model takes as given.

## 2. The mechanisms side by side

Four routes take activity out of the circulating helium or, for lift-off, put
activity back. Decay acts on everything and is not repeated in the table.

| Mechanism | What it removes or releases | Driver | Where | How TRISO-ATOPS represents it |
|---|---|---|---|---|
| **Leak** | Everything circulating, all groups alike | Pressure difference across breaks in the primary boundary; a fraction of the helium inventory per day | Out of the primary circuit into the building | Not in the upstream model. A leak sink $k_{leak}$ in `live_pools.rs` (boon-lay's own, not a port) |
| **Purification** (clean-up) | Noble gases and halogens in the HTR-10 table; also metals at lower efficiency | The fraction of helium sent through the purification system times its efficiency | The helium purification system | $k_{clean}$, a first-order rate, by group |
| **Plate-out** | Halogens and metals | Sorption and deposition on loop surfaces, depending on surface and temperature | Loop surfaces, and for part of it the steam generator | $k_{plate}$, a first-order rate, by group |
| **Lift-off** | Releases plated-out activity (and dust) back into the gas | High gas velocity and shear in a depressurisation blowdown | The same surfaces | One fraction $x_{liftoff}$ of the plate-out, released at once, in the accident path only |

Each is a first-order process with a constant rate in the model. The
equations and the pools are in
[`coolant.md`](../triso-atops/coolant.html)
and in [Step 14](./14-triso-atops.md).

### Leak

The helium inventory leaks out of the primary circuit slowly. For HTR-10 the
digital twin takes **1 % of the total volume per day**, citing Liu & Cao
(2002) section 2.4.1, with "specified to be below 1 % per day" in section
2.4.2:

$$k_{leak} = \frac{0.01}{86400\ \text{s}} \approx 1.157 \times 10^{-7}\ \text{s}^{-1}$$

([`fission_product_results`: `HTR10_PRIMARY_LEAK_PER_S`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/fission_product_release.rs)).
That is the "1 % leak assumption". It is a **bound as specified**, an upper
limit on a design allowance, not a measured leak rate. The distinction is the
subject of [Step 16](./16-building-and-bounds.md).

Upstream TRISO-ATOPS has no leak sink; `live_pools.rs` adds one, and a test
reproduces the upstream closed forms when `k_leak` is zero
([`live_pools`](../../code-review/#crates/boon-lay/src/triso_atops_fork/activities/live_pools.rs::step)).

### Purification

Gas is drawn through a clean-up train. For HTR-10 the code models the rate as
$k_{clean} = \eta \times 0.05\ \text{h}^{-1}$, a throughput cited to Yao
(2002, as the code comment gives it) times Liu & Cao's efficiency $\eta$:
99 % for I, Kr and Xe (and C, tritium), 90 % for Sr, Ag, Cs and Rb
([`htr10_pool_rates`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/fission_product_release.rs)).
Liu & Cao call the 90 % and 99 % "conservatively set". What the purification
system physically is (adsorbers, filters, temperatures) is not described in
the repo's docs I read [citation needed: maintainer to supply].

### Plate-out, and where it happens

Per-cycle deposition for HTR-10 in the same code: 30 % for Rb and Sr, 50 % for
Ag and Cs, 20 % for iodine, zero for noble gases (Liu & Cao 2002 section
2.4.1, "conservatively taken", against AVR's 50 to 90 %). A per-cycle
fraction $d$ over a cycle time $M / \dot m$ becomes a rate

$$k_{plate} = -\ln(1 - d)\ \frac{\dot m}{M}.$$

**Where.** The repo says one place for sure: the **steam generator**. Its
water-ingress stage takes the share of each element's plate-out that sits on
the steam generator as Cs 73 %, Sr 80 %, I 100 %, and washes it off if liquid
water arrives (Liu & Cao 2002 section 4.1.1.3, after Wolters et al. 1985, as
the code comment cites)
([`steam_generator_share_of_plate_out`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/water_ingress.rs)).
Which other surfaces (ducts, circulator, vessel walls) carry the rest, and how
the deposit varies with temperature along the loop, is not in the repo
[citation needed: maintainer to supply]. The model has **no position**: one
rate per group applies to the whole circuit.

### Lift-off

When the primary circuit depressurises, helium rushes out at high velocity.
It can tear loosely held plated-out activity and dust off the walls. In the
accident formula the released activity is

$$A_{released} = f_{vent}(A_{kernel} + A_{graphite}) + A_{circulating} + x_{liftoff} A_{plate}$$

([`accident_release_curies`](../../code-review/#crates/boon-lay/src/triso_atops_fork/accident/mod.rs::accident_release_curies)).
The last term is lift-off. The value of $x_{liftoff}$ for the HTR-10 stage is
**0.05, a stand-in from the NP-MHTGR case in Stoyer et al. (2026), Case A,
Table 3**; no HTR-10 value is in the repo's corpus. Liu & Cao (2002) instead
assume desorption of 2.4 times the coolant activity, for iodine and metals
(section 4.1.1.2, after INTERATOM 1989, as cited in the code comment).
**CORRECTED 2026-10-18** (Step 20 review): the two live in different code
paths. The 0.05 sits in `sembawang::htr10::plant_parameters` (the accident
comparison). The digital twin's live plant instead applies
`depressurisation_lift_off`: desorption of 2.4 times the circulating pool for
iodine and metals, a dust share of 10 % of what is plated out, and release of
the purification pool. Neither is derived from gas velocity. They are two *different assumptions*
for one process.

## 3. The inventory that stays: why operators take care

Gas that is purified is held in a trap. Atoms that plate out stay on the
surfaces for as long as they live, so long-lived metals build up over years of
operation. The pools picture in [Step 15](./15-dlofc.md) shows plate-out
growing towards a steady value, and the repo's tests show plate-out as a large
pool relative to the circulating one: in the worked end-to-end example in
[`triso-atops-fork.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/docs/triso-atops-fork.md)
a metal node gives a plate-out of 0.102 Ci (the document's own check value,
not a plant figure).

That standing inventory is on the surfaces an operator may have to open,
inspect or maintain. The repo has no occupational-dose model, so the
consequence for working practice is the maintainer's point, and there is
no model here that quantifies it [citation needed: maintainer to supply].

## Why each simplification is allowed

**Kinds of justification used below.** (a) negligible effect, with an estimate; (b) computationally too expensive to be practical; (c) bounding or conservative, with the side it errs on; (d) data not available. **Open** means this page cannot justify it.

| Simplification | Kind | Justification |
|---|---|---|
| First-order, constant-rate removal | (b) and (d) | Surface-resolved sorption is costly (Step 13) and its data are missing. Not shown negligible: **open** |
| Loop as one volume, no position | (b) | A 3D circuit model is impractical for the cases of interest. Error is **open** |
| Leak 1 % per day | (c) | A design allowance, errs high only if actual leakage is below it |
| Purification efficiency 99 % and 90 % | (c) per source, **but** direction needs checking | Higher efficiency lowers circuit inventory, so the direction depends on the quantity |
| Lift-off one fraction, released at once | (c) for the "at once", (d) for the value | The fraction is a stand-in. No gas-velocity model exists |

## What we assumed

- Every removal process is first-order with a constant rate.
- Groups, not species, set which rates apply.
- The loop is one well-mixed volume with one surface.
- Lift-off is one fraction, released at once.
- The leak and purification numbers are conservative allowances (as the
  source calls them), not measurements.

## Next question

All this was a cheap, lumped description. What would it take to compute the
transport instead of assuming it? [Step 13](./13-brute-force-cfd.md).
