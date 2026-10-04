# HTR-10: an extrapolation, and the qualification data behind it

> **Research, education and V&V only** ([`RESPONSIBLE_USE.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/RESPONSIBLE_USE.md)).
> Nothing on this page is an HTR-10 fuel performance, safety or licensing
> number.
> **Review status:** AI-assisted draft, 2026-10-04, not yet human-reviewed.

**Extended deep dive.** Covers
[`fuel_failure::htr10`](../../api/boon_lay/fuel_failure/htr10/index.html) and
[`fuel_failure::htr10::qualification`](../../api/boon_lay/fuel_failure/htr10/qualification/index.html),
which [rung 5](../../tutorials/triso-atops/failure.html#step-6-the-htr-10-numbers-and-what-they-are-not)
summarises. Tracking issue: gh:#296.

## What HTR-10 publishes, and what it does not

From IAEA-TECDOC-1382 part 2 Table 4-17, the code takes the particle geometry
(kernel radius 250 µm, buffer outer radius 340 µm, SiC 380–415 µm) and derives
the burnup (8.51 % FIMA) and residence (1080 full-power days) from published
design data, with one assumption (200 MeV per fission, ±2 % over the usual
range)
([constants](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/htr10/mod.rs#@@L:crates/boon-lay/src/fuel_failure/htr10/mod.rs:const=KERNEL_RADIUS_UM@@)).

**Not published for HTR-10, and taken by name instead:**

| Input | Value used | From |
|---|---|---|
| SiC strength `σ_oo`, modulus `m_oo` | 834 MPa, 8.02 | the report's EO 1607 pair for reactor reproductions (`STAND_IN_STRENGTH_MPA`, `STAND_IN_WEIBULL_MODULUS`) |
| fast fluence `Γ` | 1.4·10²⁵ m⁻² EDN | the report's HTR-Module and HTR-500 value (`STAND_IN_FLUENCE_E25_PER_M2`) |
| average irradiation temperature `T_B` | swept; 776 °C as the reference | HTR-Module's average; HTR-10 publishes a maximum only |

Each is a named constant whose doc says "stand-in". A number computed with
them is boon-lay fuel failure's, for a particle that is HTR-10's in geometry
and burnup and HTR-Module's in strength and fluence.

## Results, as recorded

**Normal operation, end of irradiation** (2026-09-24): $\phi_1$ =
2.8·10⁻¹⁵ (700 °C), 1.2·10⁻¹² (776 °C), 5.0·10⁻⁹ (900 °C), 1.6·10⁻⁶ (1000 °C).
Eight decades across 300 °C, which is why $T_B$ is an input and not a guess.

**Accident, 200 h isothermal, $T_B$ = 776 °C** (2026-09-24): see
[rung 5, step 6](../../tutorials/triso-atops/failure.html#step-6-the-htr-10-numbers-and-what-they-are-not)
for the table and its re-measurement.

**Why the normal-operation number must not become `f_inc`.** TRISO-ATOPS's
in-service failure fraction for normal operation is a manufacturing and
irradiation defect population, which the PANAMA-I equations take as an input.
The `htgr_sim_v1` twin once compared $\phi_1$ with a `3·10⁻⁵` placeholder
seven orders of magnitude larger; that placeholder has since been deleted
(gh:#399) because its stated provenance was false, and the twin now uses
Liu & Cao (2002)'s design irradiation failure of 5·10⁻⁴ plus $\phi_1$ plus a
chemical-attack term
([`fission_product_release.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/fission_product_release.rs#L462-L486)).

## The qualification data: German-lineage, open, and only partly used

No measured HTR-10 failure, free-uranium or release fraction exists in the
workspace's literature; the search is recorded, source by source
([`qualification.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/htr10/qualification.rs#L17-L52)).
What does exist is the German LEU UO₂ TRISO qualification record in Kugeler,
Nabielek & Buckthorpe (2017, the JRC (V)HTR-Modul safety volume), the fuel
line HTR-10's fuel descends from (the volume says so on p. 38).

- **Burn-leach defect fractions:** 8–49·10⁻⁶ expected, 20–64·10⁻⁶ at the
  one-sided upper 95 % limit, over 2.2 million particles
  (`BURN_LEACH_DEFECT_FRACTIONS`). TRISO-ATOPS's reference failure fractions
  sum to 1·10⁻⁴, about 1.6× above the worst upper-95 % figure: conservative,
  and of the right order.
- **The burnup ordering at 1600 °C**, the one falsifiable statement: 300 h at
  1600 °C, $\phi_1$ = 2.35·10⁻⁷ (4 % FIMA), 4.40·10⁻⁵ (8.51 %, HTR-10),
  6.48·10⁻⁵ (9 %), 2.60·10⁻⁴ (11 %), 1.39·10⁻³ (14 %), recorded 2026-09-24.
  The ordering is reproduced, and $\phi_1 \propto F_b^{m}$ holds exactly
  with the **irradiated** modulus $m = 6.932$: $(14/11)^{6.932} = 5.32$, the
  measured ratio. (The first prediction, 6.8× with $m = 8$, used the wrong
  row and is kept as a correction, not restated.) The 11 % compacts, where no
  failure was seen, disagree mildly.
- **What is still unchecked (gh:#383):** the free heavy-metal band `f_hm`
  (7.8–50.7·10⁻⁶) is stored and not reported beside any release, although
  noble-gas and halogen release are linear in it. (#383 also says the
  module's test `the_burnup_ordering_at_1600c_matches_and_the_level_does_not`
  fails; that was gh:#301 and #404, both closed, and the test **passed** in
  this track's run on 2026-10-04.)

**Re-run 2026-10-04** (this track's run, `develop` atop `5e802df3a4`, `--release`): 2.347·10⁻⁷, 4.397·10⁻⁵, 6.482·10⁻⁵, 2.605·10⁻⁴, 1.385·10⁻³ for 4, 8.51, 9, 11 and 14 % FIMA; the 14/11 ratio is 5.32×. Unchanged from the 2026-09-24 record, and the test passes.

## Is any of this validation?

No. The German data is a different reactor's fuel, the stand-ins are another
reactor's numbers, and nothing here is compared with a measurement made on
HTR-10 fuel. What the page can honestly say is: the model, applied to a
particle with HTR-10's geometry and burnup, gives numbers of a sensible order
against the German qualification record, and it disagrees mildly with one
compact test.
