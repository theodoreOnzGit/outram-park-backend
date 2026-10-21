# Part III — double heterogeneity: explicit TRISO vs ring-RPT in an FHR pebble

**Scope:** the doubly heterogeneous problem only. Cross-section preparation,
transport-kernel verification and the classical ICSBEP criticals are **Part II**.

**Source record:**
`crates/outram-mc-libs/verification_and_validation/ring_rpt/ring_rpt_vs_openmc.md`.

> **Rebaselined three times — 2026-09-12 (#193, F-19 threshold), 2026-09-13
> (#194, continuous thermal kernel) and 2026-09-14 (MF=6 LAW=1 continuum
> emission for MT=91/MT=16, replacing a Weisskopf evaporation stand-in).**
> Everything below is the post-MF=6 state, re-extracted 2026-09-14 from a fresh
> run of `examples/fhr_ring_rpt_endf.rs` (4000 × [30 + 80], reflective sphere
> r = 3, seed 20260910). The superseded rows are kept in the CSVs, flagged,
> because the supersession *is* part of the story: four baselines in four days,
> `+4004 → −469 → +37 → −460` pcm.
>
> **Every row here is a SINGLE DRAW.** Each arm carries ~210–240 pcm of its own
> statistics, so two runs of unchanged code differ by ~300 pcm of standard
> error and no drift smaller than that is resolved by this table. Pooling over
> seeds the way Godiva was pooled is `gh:#196` / `bn:op-awwi` and **has not been
> done**. Quote the Δ columns as this run's values, not as measurements of the
> change between baselines.
>
> **Re-extract before submission.** The numbers have moved on every day of this
> work so far, and `op-77pu` / #188 is still open beneath them.

## The result

| case | tracking | `outram-mc-libs` | OpenMC | Δ |
|---|---|---|---|---|
| explicit TRISO | delta (Woodcock) | 1.36050 ± 0.00200 | 1.36510 ± 0.00063 | **−460 pcm (2.2σ)** |
| ring-RPT | delta (Woodcock) | 1.36311 ± 0.00228 | 1.36479 ± 0.00067 | **−168 pcm (0.7σ)** |
| ring-RPT | CSG surface-tracked | 1.36752 ± 0.00210 | 1.36479 ± 0.00067 | +273 pcm (1.2σ) |
| naive homogenised | delta (Woodcock) | 1.32645 ± 0.00216 | — | — |

### Two kinds of number here — do not read them the same way

This table mixes an **agreement check** with a **physics measurement**, and
"better" means the opposite thing for each. A reader skimming for big numbers
will get this backwards.

| | wants to be | why |
|---|---|---|
| `vs OpenMC` column (−460, −168, +273) | **small** | a code-to-code agreement check against a reference. Large means *this code is wrong*. |
| `naive − explicit` (−3405) | **large** | the double-heterogeneity effect itself — the quantity the paper exists to measure. Small would mean the TRISO structure had stopped being modelled. |

Concretely: **−460 pcm is good and +4004 pcm was a bug** (the F-19 threshold
defect, 18.8σ, closed by #193). **−3405 pcm is not a bug and never was** — it
is U-238's resonances losing their spatial self-shielding when the particles
are smeared into the matrix, and it is the result. The two happen to be the
same order of magnitude, which is precisely why they get confused.

So: a fix that moves the `vs OpenMC` column toward zero is progress. A fix that
moves `naive − explicit` toward zero would be a regression, and is the standing
check that a code which had quietly stopped resolving the TRISO particles would
fail.

**The explicit pebble is now −460 pcm from the reference, at 2.2σ of the
combined statistics** — no longer the clean agreement the +37 baseline showed.
The trajectory across four days is `+4004 → −469 → +37 → −460`: the F-19
threshold fix (#193), the continuous thermal kernel (#194), then the evaluated
MF=6 continuum law.

**Read that −460 with the single-draw caveat above.** Where the MF=6 change
*is* resolved is Godiva, pooled over 64 paired seeds: the evaluated law sits at
**+214 ± 20 pcm** against the stand-in's **+319 ± 25 pcm**, a difference of
**−105 ± 32 pcm (3.3σ)**. The pebble moved the same direction, and by more than
that, on one seed per arm — consistent in sign, not established in size.

The ring-RPT arm, which is what the paper is about, is the one that barely
moved: **−168 pcm (0.7σ)**, still agreement.

**The RPT equivalence, which is what the paper is about:**

| code | RPT − explicit | σ from zero |
|---|---|---|
| `outram-mc-libs` | **+260 ± 303 pcm** | 0.86σ |
| OpenMC | −31 ± 92 pcm | 0.34σ |
| cross-code difference | +291 ± 317 pcm | 0.92σ — consistent |

Six factors landed on the reference at the F-19 fix — **η −0.00 %, f −0.01 %,
p +0.04 %, ε −0.08 %**, against +0.11 / −0.17 / +8.54 / −4.99 % before it.
Re-extract these at the continuous-kernel baseline before writing; the k rows
moved and the factor rows quoted here are from 2026-09-12.

`naive − explicit = −3405 ± 294 pcm (11.6σ)` is the double-heterogeneity
effect RPT exists to remove, and it stays **real and unchanged in character**
across every fix so far (−3191 → −3282 → −3575 → −3405). That is the check that
matters whenever a fix moves absolute k by hundreds of pcm: a code that had
quietly stopped modelling the TRISO structure would have lost this number too.

Its own movement is not a result — the −3575 → −3405 step is 170 pcm against
~300 pcm of single-draw noise, so it is consistent with no change at all. Read
it as "still ~11σ from zero", not as a trend.

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

- **The CSG surface-tracked row is the loosest at `+331 ± 223` pcm (1.5σ)**,
  against `+37` (0.2σ) delta-tracked explicit and `−116` (0.5σ) delta-tracked
  ring-RPT. It is still consistent, but do not present the three as uniformly
  excellent; say which is which.
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
