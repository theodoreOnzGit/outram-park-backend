# Part III — double heterogeneity: explicit TRISO vs ring-RPT in an FHR pebble

**Scope:** the doubly heterogeneous problem only. Cross-section preparation,
transport-kernel verification and the classical ICSBEP criticals are **Part II**.

**Source record:**
`crates/outram-mc-libs/verification_and_validation/ring_rpt/ring_rpt_vs_openmc.md`.

> **Rebaselined 2026-09-12.** This paper's numbers changed completely when the
> `+4000` pcm residual was closed (GitHub #193). Everything below is the
> post-fix state; the superseded rows are kept in the CSVs, flagged, because
> the supersession *is* part of the story.

## The result

| case | tracking | `outram-mc-libs` | OpenMC | Δ |
|---|---|---|---|---|
| explicit TRISO | delta (Woodcock) | 1.36041 ± 0.00207 | 1.36510 ± 0.00063 | **−469 pcm (2.2σ)** |
| ring-RPT | delta (Woodcock) | 1.36394 ± 0.00204 | 1.36479 ± 0.00067 | **−85 pcm (0.4σ)** |
| ring-RPT | CSG surface-tracked | 1.36140 ± 0.00229 | 1.36479 ± 0.00067 | −339 pcm (1.4σ) |
| naive homogenised | delta (Woodcock) | 1.32759 ± 0.00201 | — | — |

**The RPT equivalence, which is what the paper is about:**

| code | RPT − explicit | σ from zero |
|---|---|---|
| `outram-mc-libs` | **+353 ± 291 pcm** | 1.2σ |
| OpenMC | −31 ± 92 pcm | 0.34σ |
| cross-code difference | +384 ± 305 pcm | 1.26σ — consistent |

Six factors against the reference: **η −0.00 %, f −0.01 %, p +0.04 %,
ε −0.08 %** (they were +0.11 / −0.17 / +8.54 / −4.99 % before the fix).

`naive − explicit = −3282 pcm (11.4σ)` is the double-heterogeneity effect RPT
exists to remove, and it is **unchanged in character** from the −3191 pcm
recorded pre-fix. That is the check that the fix did not simply flatten the
physics: a code that had quietly stopped modelling the TRISO structure would
have lost that number too.

## The defect story, which is the paper's methodological contribution

The residual was `+4004` pcm for a week and was hunted through roughly twenty
mechanisms. **Every exclusion along the way was an *accuracy* statement** —
"±0.04 % vs NJOY PENDF", "resonance integral +0.00 %", "within 0.05 % of
THERMR" — none of them wrong, and none of them able to find this.

**Because the defect was not an accuracy error.** `ReconrResult::eval_mt`
clamped an MF=3 section to its endpoint value outside the tabulated grid:
correct at the top of the grid, wrong at the bottom of a **threshold** section,
where it propagates the threshold value down to zero energy. ENDF/B-VIII.0 F-19
opens MT=51 at `(115 840 eV, 0.018129 b)`, so F-19 carried a **constant
0.0224 b of inelastic scattering at every energy below 115 keV** — about 0.6 %
of its total through the whole resonance region.

`two_body_scatter` then clamped a negative outgoing CM energy to zero, so a
sub-threshold "inelastic" collision left the neutron at `E/(A+1)²` — a factor of
**394** for fluorine, **six lethargy units in one collision**. Roughly one F-19
collision in 170 was teleporting neutrons straight past the U-238 resonances.

F-19's inelastic cross section is faithful to its tape to 1e-7 *everywhere it
exists*. What was wrong was the 0.0224 b where it should not have existed at
all — a region no accuracy comparison was looking at, because both sides of
every comparison agreed there was nothing there.

### What found it: pricing, not checking

`mechanism_pricing.csv`. Run the case twice, once with the mechanism switched
off, and read the reactivity worth. An accuracy statement bounds nothing.

| switched off | Δk | p vs OpenMC |
|---|---|---|
| graphite S(α,β) → free gas | −127 ± 337 pcm | +8.54 → +8.18 % |
| MF=4 elastic angle → isotropic CM | −81 ± 317 pcm | +8.54 → +8.49 % |
| free-gas target motion → at rest (**positive control**) | −2242 ± 323 pcm | +8.54 → +6.03 % |
| inelastic channel, all nuclides | −4190 ± 335 pcm | +8.54 → −1.03 % |
| **inelastic channel, F-19 only** | **−4033 ± 333 pcm** | **+8.54 → −0.45 %** |

**The positive control row is the methodological point.** A table of null
results is only worth reading if the instrument can produce a non-null one, so
a known defect was deliberately reinstated to prove the measurement had power.
Report it as a control, not as a candidate.

The second thing that found it was an **invariant**: can a threshold reaction
occur below its threshold? Neither of those needs a reference code — which is
the transferable lesson, since most codes being verified do not have an OpenMC
to lean on.

### Why only this case ever saw it

The clamp applied to every threshold MT on every HIGH-tier nuclide, but it only
leaks where the evaluation opens its threshold section above zero. **F-19 is the
only nuclide in this repository's reference data that does** — U-238, U-235,
C-12, O-16, Li-7, Be-9 and Si-28 all start theirs at exactly 0.0 b. So Godiva,
HST-009, LCT-008, HTR-10 and both light-water cases were untouched, and the FHR
pebble — **70 % FLiBe by volume, four F per molecule** — carried the whole error
alone.

## The second finding: an agreeing number is not evidence of a correct model

Before the F-19 fix, and before two earlier fixes, `RPT − explicit` read
`+163 ± 316` pcm — 0.58σ, a result most authors would have published. It was two
defects of opposite sign cancelling: missing free-gas target motion (`op-50vu`)
and a pebble packed 2.6 % over the deck's own definition (`op-8l2e`). Fixing one
exposed the other at `+1130 ± 319` (3.54σ).

`ring_rpt_equivalence_history.csv` carries that sequence. It belongs in the
abstract alongside the F-19 story — together they say the same thing from two
directions: **agreement is not evidence, and accuracy is not sufficiency.**

## Files

| File | Contents |
|---|---|
| `openmc_absolute_comparison.csv` | absolute k vs OpenMC, both tracking methods; superseded rows flagged |
| `rpt_equivalence_cross_code.csv` | RPT − explicit, this code vs OpenMC |
| `mechanism_pricing.csv` | the worth measurement that localised the defect |
| `ring_rpt_equivalence_history.csv` | the cancelling-defects sequence |
| `mechanism_elimination.csv` | 21 accuracy-based exclusions — the trail that did *not* find it |

Present `mechanism_elimination.csv` **as the negative result it is.** Twenty-one
correct exclusions that could not find the defect is the setup for why pricing
was needed; dressing it up as the diagnosis would misrepresent how this went.

Its ICSBEP and NJOY rows are results **imported from Part II** — caption them as
cited, not re-derived.

## Also belongs here

- **Two transport defects this work surfaced**, both fixed: GH #168, a
  concentric-sphere CSG leak root-caused to cell membership being re-derived
  from a floating-point sense evaluation at a point exactly on the surface; and
  GH #169, charged-particle disappearance channels (MT 103–117) missing from
  absorption — Li-6(n,t)α returned 0.04 b against a true 938 b.
- **GH #192**, found while reading the inelastic channel closely: the MT=91
  continuum ignored its Q-value and emitted above the kinematic bound in 22 % of
  draws just above threshold.
- **Spatial self-shielding was excluded by an oracle**, not by assumption:
  `physics::collision_probability` gives exact first-flight collision
  probabilities for concentric spheres, verified to 1e-13 against the closed
  form. On the real annulus the lumping effect is `−0.97 % ± 0.72 %`, i.e.
  `−32 ± 24` pcm — and the sign is backwards for the hypothesis it was testing.
- **Two independent tracking methods on the same geometry** — the delta and CSG
  rows above, which is what licenses delta tracking for doubly heterogeneous
  media.
- **The RPT inner radius is a fitted, code-dependent parameter.** 1.493359375 cm
  was fitted against OpenMC; `--search-rpt-radius` solves for this code's own.

## Publication order: Part II first, by choice

The technical dependency is gone — Part III's residual was F-19, LCT-008
contains no fluorine, and LCT-008's `+2950 ± 61` pcm remains open as a separate
defect (bead `op-4ic7`). **Part II still goes first** (maintainer decision,
2026-09-12): it is an editorial choice now rather than a constraint, keeping the
series in the order a reader wants and matching Part I's numbering. Cite Part II
as context, not as the explanation of this paper's result.

## Caveats this paper must carry

Beyond the workspace-wide ones in `../README.md`:

- **`−469 ± 216` pcm on explicit TRISO is 2.2σ, not agreement.** The ring-RPT
  row (`−85`, 0.4σ) is the clean one. Report the two distinctly.
- **σ ≈ 200 pcm** at 4000 × [30 + 80]. Several comparisons here are "consistent
  with zero", not "agree to X pcm". State what the statistics can resolve.
- **Thermal-kernel defects remain open underneath this result** — GH #188 is
  mitigated, not closed (ξ still −2.79 % at 128 bins), and `op-x77y` has the
  graphite emission grid too coarse. The F-19 fix did not touch either.
- **Two modelling approximations stand**: TRISO coating layers resolved by
  nearest-centre + radius rather than exact CSG, and the RPT inner radius is a
  fitted parameter taken from a fit against OpenMC.
- **No human V&V sign-off.** Both bookkeeping axes are still ❌.

## Regenerating

```bash
cargo run --release -p outram-mc-libs --features endf-pebble-cases \
    --example fhr_ring_rpt_endf
```

**Not re-run to build this dataset** — extracted from the committed record.
