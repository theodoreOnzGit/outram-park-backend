# Case study: correct physics is the default

## The problem

A cross-section evaluation carries more physics than a simple transport code
uses. Two examples matter for thermal and intermediate-spectrum systems:

- **Unresolved-resonance (URR) self-shielding.** Above the resolved resonances
  the evaluation gives *average* resonance parameters. Probability tables
  (NJOY's PURR, [MacFarlane et al., 2017](#ref-njoy2016)) recover the self-shielding a fluctuating cross section causes.
- **Resonance elastic scattering (DBRC).** A neutron scattering off a heavy
  nucleus near a resonance sees the target's thermal motion through a strongly
  energy-dependent cross section. The constant-cross-section free-gas kernel gets
  this wrong, and DBRC corrects it.

Both were implemented in this crate in September 2026, as **opt-in builders**.
The question this chapter is about: what happens to physics nobody turns on?

## What happened

On 2026-09-20 a hunt for the causes of this crate's ICSBEP [(Briggs et al., 2003)](#ref-briggs2003international) residuals found that
URR and DBRC **both defaulted off**, and that **no ICSBEP benchmark example enabled
them**. Every recorded residual for Godiva, Jemima, HST-009 and LCT-008 had been
measured against a model missing both. Nothing had flagged it: a missing physics
term does not raise an error, it just shifts `k_eff`.

The defaults were also **mis-read during the hunt itself**. A search for
`with_urr_probability_tables` found hits in `lct008_keff.rs`, and that was taken
as the feature being active, when three lines away it read
`urr: args.iter().any(|a| a == "--urr")`: off unless asked for. **Counting a
symbol's presence is not checking a default.** The line is still in
[`examples/lct008_keff.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lct008_keff.rs#@@L:crates/outram-mc-libs/examples/lct008_keff.rs:text=urr:+args.iter().any@@),
but it no longer decides anything. The constructor that example calls now applies
both terms itself (next section), so the flag only rebuilds tables the nuclide
already carries. The example's own text said both were "default OFF" until
2026-10-03, when it was corrected in place; its 2026-09-16 table is the state of
that day, before the default changed.

The full account is in the workspace
[`CLAUDE.md`, "Correct physics is the DEFAULT SETTING"](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/CLAUDE.md#L145-L197),
which turned it into a rule for every high-fidelity crate.

## The fix: one funnel, defaults on, ablation explicit

Every ENDF construction path (`from_endf_file`, `from_tape`) goes through one
function, and that function now applies both
([`nuclide.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L2650-L2676)):

```rust,ignore
{{#include ../../../src/material/nuclide.rs:2650:2676}}
```

Three design points carry over to any crate:

1. **Apply it at the single point every path funnels through**, so no
   constructor can be missed.
2. **Ablation is a visible, named act**:
   [`without_urr_probability_tables`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L1185-L1188)
   and [`without_dbrc`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#L1302-L1305).
   An ablation that is the default is not an ablation. It is a missing term.
3. **Runtime is stated, not used as an excuse.** PURR roughly doubled nuclide
   construction on Godiva (64.0 s → 140.7 s for three actinides, measured
   2026-09-20), and the cost was accepted.

## Pinning the default with a test

A default nothing asserts will drift back off.
[`tests/correct_physics_is_default.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/tests/correct_physics_is_default.rs)
builds nuclides through the **ordinary** constructors and asserts the physics is
present. It checks that U-238 from ENDF carries URR tables over roughly 20–149 keV
and applies delayed spectra
([L72-L107](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/tests/correct_physics_is_default.rs#L72-L107)).

It also shows how such a test can itself be incomplete. It first covered only the
ENDF route, while `from_ace` still set `urr: None`. **Two routes through one crate
carried different physics**, and the test written to prevent exactly that drift
kept passing. A rule pinned on one construction path is pinned on none. The ACE
case was added under GitHub #307
([L17-L66](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/tests/correct_physics_is_default.rs#L17-L66)),
and it also checks that the tables **change the cross section**, not merely that
they exist. DBRC is deliberately not required on the ACE route, and the test says
why: an ACE table broadened to its own temperature carries no 0 K elastic data to
build it from.

## The numbers got worse, and that is the point

Turning the physics on did not flatter the benchmarks. The workspace `CLAUDE.md`
records that enabling URR moved Jemima from −253 to −395 pcm, and that DBRC+URR
moved LCT-008 from +165 to +139 pcm, a shift not resolved at 0.7σ. Correct
physics is not chosen by whether it improves a comparison. A model missing a term
that happens to agree with a benchmark is not evidence of anything.

Pricing a term properly takes a **paired** study: same seeds, one mechanism
changed, many seeds.
[`examples/lct008_urr_dbrc_ablation.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/examples/lct008_urr_dbrc_ablation.rs#L1-L60)
is one. It writes down its predicted sign and magnitude **before** running, so the
result is capable of contradicting the prediction, and it reports a **bound**
where the statistics cannot resolve the effect, rather than a number.

<!-- references:begin -->
## References

<p class="csl-entry" id="ref-briggs2003international" style="padding-left: 2em; text-indent: -2em;">Briggs, J. B., Scott, L., &#38; Nouri, A. (2003). The international criticality safety benchmark evaluation project. <i>Nuclear Science and Engineering</i>, <i>145</i>(1), 1–10.</p>

<p class="csl-entry" id="ref-njoy2016" style="padding-left: 2em; text-indent: -2em;">MacFarlane, R. E., Muir, D. W., Boicourt, R. M., Kahler, A. C., &#38; Conlin, J. L. (2017). <i>The NJOY Nuclear Data Processing System, Version 2016</i> (Technical Report LA-UR-17-20093). Los Alamos National Laboratory. https://www.osti.gov/biblio/1338791</p>

<!-- references:end -->
