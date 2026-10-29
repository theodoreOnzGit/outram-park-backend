//! The Unit 1 comparison against BSAF Phase II, and the ablation table
//! (gh:#1032, plan `docs/fukushima-melcor-gap-plan.md` item F1).
//!
//! Runs the recorded sequence headless, measures the quantities that have a
//! BSAF counterpart, and sets each beside its band **and the band's tier**,
//! from `crates/kovan-literature/derived/nea7525-bsaf-fukushima-daiichi-bands.md`
//! (§1 and §6). The tier says what a comparison can mean:
//!
//! - **A** measured: missing it means the model is wrong.
//! - **B** well-constrained code consensus: a real check.
//! - **C** wide code spread, read off a figure: context only. Being inside
//!   proves little.
//! - **D** known to be wrong: never a target (none is used here).
//!
//! **The bands are checks, never inputs.** Nothing in the model is tuned
//! toward them (root `CLAUDE.md`, "Get the PROCESS right first"). A miss is
//! a result to report, and the ablation rows say how much of it each
//! modelled mechanism carries.
//!
//! Acknowledgement: OECD Nuclear Energy Agency, *Benchmark Study of the
//! Accident at the Fukushima Daiichi Nuclear Power Plant*, NEA No. 7525,
//! © OECD 2021. Facts and numbers only, cited with acknowledgement.

use super::params::{Params, Tag};
use super::{Mechanisms, Mode, Plant, DT_S};

/// What a band can mean (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    A,
    B,
    C,
}

impl Tier {
    fn label(self) -> &'static str {
        match self {
            Tier::A => "A (measured)",
            Tier::B => "B (code consensus)",
            Tier::C => "C (context only)",
        }
    }
}

/// One BSAF Phase II band for Unit 1.
#[derive(Clone, Copy, Debug)]
pub struct Band {
    pub name: &'static str,
    pub low: f64,
    pub high: f64,
    pub unit: &'static str,
    pub tier: Tier,
    /// Where it is in the derived record.
    pub source: &'static str,
}

/// The Unit 1 bands, in the order of [`Metrics::values`].
pub const BANDS: [Band; 6] = [
    Band { name: "RPV lower-head failure", low: 7.45, high: 17.16, unit: "h", tier: Tier::B, source: "§1.3, Table 2.2" },
    Band { name: "RPV depressurised (below 2 MPa)", low: 5.0, high: 12.0, unit: "h", tier: Tier::A, source: "§1.1 (measured between 5 and 12 h)" },
    Band { name: "In-vessel H2 at 25 h", low: 400.0, high: 1310.0, unit: "kg", tier: Tier::C, source: "§1.4, Fig. 2.2 (figure-read)" },
    Band { name: "Drywell pressure, peak 0-25 h", low: 0.8, high: 1.0, unit: "MPa", tier: Tier::C, source: "§1.5, Fig. 2.1b (calculated, figure-read)" },
    Band { name: "Wetwell vent flowing", low: 23.5, high: 24.68, unit: "h", tier: Tier::A, source: "§1.1 (\"vented ~24 h\", closed 24.68 h; 23.5 h is this comparison's reading of ~24 h)" },
    Band { name: "Reactor-building explosion", low: 24.83, high: 24.83, unit: "h", tier: Tier::A, source: "§1.1 (observed 24.83 h)" },
];

/// The pressure below which the RPV counts as depressurised, Pa. A
/// definition of this comparison, not a plant value: Unit 1's vessel fell
/// from about 7 MPa to under 1 MPa in the measured window, so any threshold
/// well between the two picks the same moment.
const DEPRESSURISED_PA: f64 = 2.0e6;

/// How long the comparison runs, s: past the recorded explosion.
pub const COMPARE_END_S: f64 = 26.0 * 3600.0;

/// What the model gives for each band. `None` where the event never
/// happens in the run.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Metrics {
    pub lower_head_h: Option<f64>,
    pub depressurised_h: Option<f64>,
    pub h2_25h_kg: f64,
    pub p_dw_peak_mpa: f64,
    pub vent_h: Option<f64>,
    pub explosion_h: Option<f64>,
}

impl Metrics {
    /// The values in the order of [`BANDS`].
    pub fn values(&self) -> [Option<f64>; 6] {
        [
            self.lower_head_h,
            self.depressurised_h,
            Some(self.h2_25h_kg),
            Some(self.p_dw_peak_mpa),
            self.vent_h,
            self.explosion_h,
        ]
    }
}

/// Run the recorded sequence with `mechanisms` and measure every band's
/// quantity. Deterministic, like the headless mode.
pub fn measure(mechanisms: Mechanisms) -> Metrics {
    measure_with(mechanisms, Params::default())
}

/// [`measure`] with the plant's inputs set to `prm` (gh:#1033).
pub fn measure_with(mechanisms: Mechanisms, prm: Params) -> Metrics {
    let mut plant = Plant::with_params(Mode::Historical, mechanisms, prm);
    let (mut core, mut cont) = plant.parts();
    let mut m = Metrics::default();
    let steps = (COMPARE_END_S / DT_S).round() as usize;
    for _ in 0..steps {
        plant.step_inline(&mut core, &mut cont);
        let f = plant.frame();
        let h = f.t_s / 3600.0;
        if f.core.lower_head_failed && m.lower_head_h.is_none() {
            m.lower_head_h = Some(h);
        }
        if f.vessel.p_pa < DEPRESSURISED_PA && m.depressurised_h.is_none() {
            m.depressurised_h = Some(h);
        }
        if h <= 25.0 {
            m.h2_25h_kg = f.core.h2_total_kg;
            m.p_dw_peak_mpa = m.p_dw_peak_mpa.max(f.containment.p_dw_pa / 1e6);
        }
        if f.containment.vent_flowing && m.vent_h.is_none() {
            m.vent_h = Some(h);
        }
        if f.containment.rb_exploded && m.explosion_h.is_none() {
            m.explosion_h = Some(h);
        }
    }
    m
}

/// Where a value sits against its band.
fn verdict(v: Option<f64>, b: &Band) -> &'static str {
    match v {
        None => "never happens",
        Some(x) if x < b.low => "below",
        Some(x) if x > b.high => "above",
        Some(_) => "inside",
    }
}

fn fmt(v: Option<f64>) -> String {
    v.map_or("never".to_string(), |x| format!("{x:.2}"))
}

/// The ablation runs: the baseline, then each mechanism off alone.
pub fn ablations() -> Vec<(&'static str, Mechanisms)> {
    let all = Mechanisms::default();
    vec![
        ("baseline (all on)", all),
        ("without hot leak", all.without_hot_leak()),
        ("without IC H2 blanketing", all.without_ic_h2_blanketing()),
        ("without T-dependent c_p", all.without_temperature_dependent_properties()),
        ("without lower-head wall", all.without_lower_head_wall()),
        ("without rod collapse", all.without_rod_collapse()),
        ("without core radiation", all.without_core_radiation()),
        ("without level swell", all.without_level_swell()),
        ("without decay mix", all.without_decay_mix()),
        ("without capture correction", all.without_capture_correction()),
        ("without actinide decay", all.without_actinide_decay()),
        ("without mixture choking", all.without_mixture_choking()),
    ]
}

/// How one input moves the metrics: elasticities `(dm/m) / (dp/p)` by
/// central difference, the input scaled by `1 - rel` and `1 + rel` about
/// its default with every mechanism on.
///
/// **A ranking instrument, not an uncertainty analysis.** It is local (a
/// derivative at the defaults), and a relative step is not an input's real
/// uncertainty: 1 % of an activation energy is a large change, 1 % of a
/// valve area a small one. It says which unverified inputs the answer leans
/// on hardest, i.e. which to check first against the sources (plan item
/// F13). It is never used to choose a value (plan rule 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Elasticity {
    pub name: &'static str,
    pub tag: Tag,
    pub default: f64,
    /// `None` when the lower head does not fail in all three runs.
    pub lower_head: Option<f64>,
    pub h2: f64,
    pub p_dw: f64,
    /// Whether the vent or the explosion happens differs between a perturbed
    /// run and the default one (~~appears; neither happened at the
    /// defaults~~: since gh:#1036 the vent does).
    pub event_changes: bool,
}

impl Elasticity {
    /// The largest of the three magnitudes, for ranking.
    pub fn largest(&self) -> f64 {
        self.lower_head.unwrap_or(0.0).abs().max(self.h2.abs()).max(self.p_dw.abs())
    }
}

/// The [`Elasticity`] of the input `name` (from [`Params::INPUTS`]) for a
/// relative step `rel`, given the default run `base`. `None` for an unknown
/// name. Runs the sequence twice.
pub fn elasticity(name: &'static str, rel: f64, base: &Metrics) -> Option<Elasticity> {
    let &(_, tag) = Params::INPUTS.iter().find(|(n, _)| *n == name)?;
    let d = Params::default();
    let default = d.get(name)?;
    let run = |f: f64| {
        let mut p = d;
        p.set(name, default * f);
        measure_with(Mechanisms::default(), p)
    };
    let (lo, hi) = (run(1.0 - rel), run(1.0 + rel));
    let e = |l: f64, h: f64, b: f64| if b == 0.0 { 0.0 } else { (h - l) / (2.0 * rel * b) };
    let lower_head = match (lo.lower_head_h, hi.lower_head_h, base.lower_head_h) {
        (Some(l), Some(h), Some(b)) => Some(e(l, h, b)),
        _ => None,
    };
    let happens = |m: &Metrics| (m.vent_h.is_some(), m.explosion_h.is_some());
    let event_changes = [lo, hi].iter().any(|m| happens(m) != happens(base));
    Some(Elasticity {
        name,
        tag,
        default,
        lower_head,
        h2: e(lo.h2_25h_kg, hi.h2_25h_kg, base.h2_25h_kg),
        p_dw: e(lo.p_dw_peak_mpa, hi.p_dw_peak_mpa, base.p_dw_peak_mpa),
        event_changes,
    })
}

/// The elasticity table as Markdown, largest effect first.
pub fn sensitivity_report(rows: &[Elasticity]) -> String {
    let mut rows = rows.to_vec();
    rows.sort_by(|a, b| b.largest().total_cmp(&a.largest()).then(a.name.cmp(b.name)));
    let mut out = String::from("| Input | Tag | Default | lower-head time | H2 at 25 h | drywell peak | vent or explosion changes |\n|---|---|---|---|---|---|---|\n");
    for r in rows {
        let lh = r.lower_head.map_or("no failure".to_string(), |x| format!("{x:+.3}"));
        out.push_str(&format!(
            "| `{}` | {:?} | {:.6e} | {} | {:+.3} | {:+.3} | {} |\n",
            r.name,
            r.tag,
            r.default,
            lh,
            r.h2,
            r.p_dw,
            if r.event_changes { "yes" } else { "no" }
        ));
    }
    out
}

/// The comparison and ablation tables, as Markdown.
pub fn report(runs: &[(&str, Metrics)]) -> String {
    let mut out = String::from("| Quantity | Tier | BSAF band | Source |");
    for (name, _) in runs {
        out.push_str(&format!(" {name} |"));
    }
    out.push_str("\n|---|---|---|---|");
    out.push_str(&"---|".repeat(runs.len()));
    out.push('\n');
    for (i, b) in BANDS.iter().enumerate() {
        let band = if b.low == b.high { format!("{:.2} {}", b.low, b.unit) } else { format!("{:.2}-{:.2} {}", b.low, b.high, b.unit) };
        out.push_str(&format!("| {} | {} | {} | {} |", b.name, b.tier.label(), band, b.source));
        for (_, m) in runs {
            let v = m.values()[i];
            out.push_str(&format!(" {} ({}) |", fmt(v), verdict(v, b)));
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The uncalibrated baseline, pinned.** Methodology: run the recorded
    /// Unit 1 sequence to 26 h with every mechanism on and measure the six
    /// BSAF quantities. Nothing in the model was tuned toward the bands.
    ///
    /// Results (2026-10-29, measured on branch `claude/digital-twin-engine-vfseuf`
    /// at the physics of `28ff10e31`; `--compare` prints the same table):
    ///
    /// | Quantity | Model | BSAF band (tier) | Verdict |
    /// |---|---|---|---|
    /// | lower-head failure | ~~5.39 h~~ ~~5.41 h~~ ~~5.39 h~~ ~~5.40 h~~ ~~5.54 h~~ ~~5.41 h~~ ~~5.40 h~~ ~~5.59 h~~ ~~4.67 h~~ 4.51 h | 7.45-17.16 h (B) | early |
    /// | RPV depressurised < 2 MPa | ~~5.39 h~~ ~~5.41 h~~ ~~5.39 h~~ ~~5.40 h~~ ~~5.54 h~~ ~~5.41 h~~ ~~5.40 h~~ ~~5.59 h~~ ~~4.67 h~~ 4.51 h | 5-12 h (A) | ~~inside~~ below, for the wrong reason |
    /// | in-vessel H2 at 25 h | ~~126.62 kg~~ ~~124.15 kg~~ ~~122.31 kg~~ ~~120.73 kg~~ ~~119.95 kg~~ ~~180.57 kg~~ ~~216.06 kg~~ ~~182.90 kg~~ ~~283.76 kg~~ 286.63 kg | 400-1310 kg (C) | low |
    /// | drywell peak 0-25 h | ~~0.477 MPa~~ ~~0.476 MPa~~ ~~0.475 MPa~~ ~~0.508 MPa~~ ~~0.532 MPa~~ ~~0.510 MPa~~ ~~0.559 MPa~~ 0.562 MPa | 0.8-1.0 MPa (C) | low |
    /// | wetwell vent | ~~never~~ ~~25.12 h~~ 23.73 h | ~24 h (23.5-24.68 h) (A) | ~~missed~~ ~~late~~ inside, regime-fragile |
    /// | building explosion | never | 24.83 h (A) | missed |
    ///
    /// The RPV depressurises *at* the lower-head failure: the breach lets
    /// the pressure go. Most BSAF analyses have an RCS leak at 3-4 h doing
    /// it (§1.1), so "inside" here is not agreement on the mechanism.
    ///
    /// Ablations, measured with constant `c_p` (before gh:#1038): without
    /// the `[A]` hot leak, H2 fell to 118.04 kg (the leak carried 8.6 kg of
    /// it; 2.5 kg with MATPRO `c_p`, 124.15 -> 121.64) and the lower head
    /// failed 0.01-0.04 h earlier; without IC hydrogen blanketing, **no change at all**,
    /// because both condensers stop at the station blackout (0.85 h), long
    /// before any hydrogen exists. Both are recorded results, not defects.
    ///
    /// **Moved 2026-10-29 by gh:#1038** (MATPRO temperature-dependent fuel
    /// and cladding heat capacities, from OFFBEAT): lower head 5.391 ->
    /// 5.409 h, H2 126.62 -> 124.15 kg, drywell peak 0.4767 -> 0.4761 MPa.
    /// Predicted "small, of either sign"; it was. The struck values are the
    /// constant-`c_p` model, which the
    /// `without_temperature_dependent_properties` ablation still reproduces
    /// exactly (pinned below).
    ///
    /// **Moved again 2026-10-29 by gh:#1038** (the Zr-steam reaction heat:
    /// ~~6.45 MJ/kg, `[U]`~~ -> 6.763 MJ/kg, OFFBEAT's value derived from
    /// standard formation enthalpies): lower head 5.409 -> 5.386 h, H2
    /// 124.15 -> 122.31 kg, drywell peak 0.4761 -> 0.4749 MPa. Predicted
    /// before the run: +4.9 % reaction heat, lower head earlier by under
    /// 0.1 h, H2 within 5 % of either sign. Measured: earlier by 0.023 h, H2
    /// -1.5 %. An input correction, not a mechanism, so it has no ablation;
    /// the constant-`c_p` ablation below now carries the new reaction heat
    /// too, so it no longer reproduces the pre-gh:#1038 model.
    ///
    /// **Moved again 2026-10-29 by gh:#1046** (energy conservation:
    /// Hess's law at the oxidising node, the new hydrogen heated in the gas
    /// stream, and the vessel paying for the hydrogen it sends out): lower
    /// head 5.386 -> 5.399 h, H2 122.31 -> 120.73 kg, drywell peak 0.4749
    /// -> 0.4746 MPa. Predicted on the issue before the run: lower head
    /// later and small, H2 small with an uncertain sign, drywell slightly
    /// lower. All three held. A conservation fix, not a mechanism, so it
    /// has no ablation.
    ///
    /// **Moved again 2026-10-29 by gh:#1034** (the lower-head wall: the head
    /// fails on its inner node or by creep, after MELCOR's public COR model,
    /// instead of on the debris temperature): lower head 5.399 -> 5.543 h,
    /// H2 120.73 -> 119.95 kg, drywell peak 0.4746 -> 0.4753 MPa. Predicted
    /// before the run: later by 0.3-1.5 h. **Missed**: later by 0.14 h.
    /// The estimate heated the whole wall as one lump; the criterion is the
    /// inner node, a 2.75 cm half-cell that tracks the debris about 80 K
    /// behind. See `verification_and_validation/bwr_mark1/lower-head-wall.md`,
    /// with its mesh study. The ablation `without_lower_head_wall`
    /// reproduces the previous baseline exactly (pinned below).
    ///
    /// **Moved again 2026-10-29 by gh:#1035** (Zircaloy's heat of fusion,
    /// 2.25e5 J/kg at 2098 K, and UO2's, from MELCOR's MP tables citing
    /// MATPRO; the enthalpy tables had none): lower head 5.543 -> 5.413 h,
    /// H2 119.95 -> 180.57 kg (+51 %), drywell 0.4753 -> 0.5080 MPa.
    /// Predicted before the run: lower head slightly later, H2 slightly
    /// up. **The lower-head sign was wrong, and H2 was far larger.** Traced:
    /// - node-steps on the 2098 K plateau went from 138 to 12 666, and
    ///   Urbanic-Heidrick oxidation is fast there (H2 at 4 h, 83 -> 141 kg);
    /// - the extra oxidation heat, and the fusion heat the quenched debris
    ///   gives the water, dry the plenum 464 s sooner (16 989 -> 16 525 s),
    ///   so the wall heats sooner.
    ///
    /// The constant-`c_p` ablation has no fusion (it is the old property
    /// model) and does not move.
    ///
    /// **Moved again 2026-10-29 by gh:#1035** (MELCOR's rod collapse: the
    /// shell breaches at 2400 K; a bare rod collapses at 2500 K or by
    /// time-at-temperature damage, any rod at 3100 K; replacing the 2100 K
    /// whole-node slump): lower head 5.413 -> 5.395 h, H2 180.57 -> 216.06
    /// kg (+19.6 %), drywell 0.5080 -> 0.5319 MPa. Predicted before the run:
    /// H2 +10-50 % (held), lower head later by 0.1-1 h with low confidence
    /// (**missed**: about zero, slightly earlier). Traced: relocation 217 s
    /// later, but the extra oxidation heat dries the plenum 721 s sooner,
    /// and the head fails about an hour after dryout either way. Three
    /// changes running, the lower-head time has followed plenum dryout,
    /// which points at the lower-plenum quench and dryout model next. The
    /// ablation `without_rod_collapse` reproduces the previous baseline
    /// exactly (pinned below).
    ///
    /// **Moved again 2026-10-29 by gh:#1037** (in-core radiation and the
    /// shroud, after MELCOR's public COR radiation model: gray-body exchange
    /// between neighbouring rings and levels and from the outer ring to a
    /// 5 cm steel shroud, exchange factors 0.1, Zircaloy emissivity from
    /// the oxide): lower head 5.395 -> 5.589 h, H2 216.06 -> 182.90 kg
    /// (-15 %), drywell 0.5319 -> 0.5104 MPa. Predicted before the run:
    /// lower head later by 0-0.3 h, H2 down 0-20 %. **Both held.** The
    /// ablation `without_core_radiation` reproduces the previous baseline
    /// exactly (pinned below).
    ///
    /// **Moved again 2026-10-29 by gh:#1036** (the core sees the swollen
    /// two-phase level, after MELCOR's CVH pool model: bubble rise 0.3 m/s,
    /// mean void J/(2 v0) capped at 0.4): lower head 5.589 -> 4.647 h, H2
    /// 182.90 -> 285.20 kg, drywell 0.5104 -> 0.5588 MPa, and the wetwell
    /// vent now flows at 25.11 h. Predicted before the run: lower head later
    /// by 0.05-0.5 h, H2 little changed. **Missed, sign and size.** The
    /// swell itself is centimetres; it tips the run into a different
    /// **regime**. Traced: a large debris slug collapses into the still-wet
    /// plenum at once, the quench pushes the vessel to 10 MPa, and the
    /// quench steam feeds an oxidation burst in nodes that were
    /// steam-starved. The regime is not a knife-edge at the default: a scan
    /// of the bubble rise velocity gave lower head 4.63-4.75 h and H2
    /// 280-318 kg for every value from 0.20 to 0.35 m/s, and 5.59 h and
    /// 191 kg at 0.40 m/s (the no-swell regime). The ablations split across
    /// the two regimes too (no wall: early; no rod collapse, constant c_p:
    /// late). **The lower-head time is regime-fragile**, so no comparison
    /// with its band is robust until the slug's size stops deciding the run
    /// (candling, support holdup). The swell stays on: it is MELCOR's
    /// default physics, and the fragility is a finding about the model.
    /// The no-swell ablation reproduces the previous baseline exactly
    /// (pinned below).
    ///
    /// The early regime also took a node's cladding past the enthalpy
    /// tables' 3200 K top (the cladding, heated by its own oxidation, above
    /// its fuel). Fixed in the same change: a rod also falls when its
    /// cladding reaches ZrO2's melting point, 2990 K (MELCOR MP Table 7),
    /// since this model cannot candle the molten oxide away. That moved
    /// the swollen-level run 4.647 -> 4.668 h, H2 285.20 -> 283.76 kg, vent
    /// 25.106 -> 25.123 h, the regime unchanged; the figures above are with
    /// it.
    ///
    /// **Moved again 2026-10-29 by gh:#1041** (decay heat in the ANS-1979
    /// form as MELCOR applies it: U-235, Pu-239 and U-238 fission products
    /// after 5.05e7 s at MELCOR's BWR shares, the capture correction G(t),
    /// U-239 and Np-239). Decay power, measured before any plant run, against
    /// the old U-235-equilibrium source: x1.007 at 1 h, 1.058 at 3 h, 1.078
    /// at 5 h, 1.101 at 10 h, 1.108 at 25 h (predicted +5 to +10 % over
    /// 3-25 h; slightly over at 25 h). Plant: lower head 4.668 -> 4.509 h,
    /// H2 283.76 -> 286.63 kg, drywell 0.5585 -> 0.5622 MPa, and the vent
    /// 25.12 -> 23.73 h, **now inside its Tier A band**. Predicted: lower
    /// head earlier by 0-0.5 h, H2 up 0-20 %. Both held. The vent landing in
    /// its band was not predicted and nothing was tuned toward it; given the
    /// regime fragility above it is reported as inside, not as agreement.
    /// All three decay switches off reproduces the previous baseline exactly
    /// (pinned below).
    ///
    /// **Moved again 2026-10-29** (choked flow of the vessel's hydrogen-steam
    /// gas from its own molar mass, `sqrt(M_mix/M_H2O)` on the steam
    /// tables' flux, after MELCOR's gas choking): lower head 4.5092 ->
    /// 4.5089 h, H2 286.63 -> 287.47 kg, drywell 0.5622 -> 0.5629 MPa, vent
    /// unchanged. Predicted: lower head about unchanged, H2 little changed.
    /// Held. `without_mixture_choking` reproduces the previous baseline
    /// exactly (pinned below).
    ///
    /// When a plan item moves these numbers, this test changes in the same
    /// commit with the old value, the new one and why (never a quiet
    /// repair).
    #[test]
    fn the_uncalibrated_baseline_is_pinned() {
        let m = measure(Mechanisms::default());
        let near = |a: Option<f64>, b: f64, tol: f64| a.is_some_and(|x| (x - b).abs() <= tol);
        assert!(near(m.lower_head_h, 4.5089, 0.001), "{m:?}");
        assert!(near(m.depressurised_h, 4.5089, 0.001), "{m:?}");
        assert!((m.h2_25h_kg - 287.47).abs() < 0.01, "{m:?}");
        assert!((m.p_dw_peak_mpa - 0.5629).abs() < 0.0001, "{m:?}");
        // ~~No vent, no explosion~~: since gh:#1036 the wetwell vent flows,
        // at ~~25.12 h~~ 23.73 h since gh:#1041 (BSAF: 23.5-24.68 h, Tier A).
        assert!(near(m.vent_h, 23.7336, 0.001) && m.explosion_h.is_none(), "{m:?}");
        // The steam-only choking ablation is the model before the mixture
        // correction, exactly.
        let mc = measure(Mechanisms::default().without_mixture_choking());
        assert!(near(mc.lower_head_h, 4.5092, 0.001), "{mc:?}");
        assert!((mc.h2_25h_kg - 286.63).abs() < 0.01, "{mc:?}");
        assert!((mc.p_dw_peak_mpa - 0.5622).abs() < 0.0001, "{mc:?}");
        // All three decay switches off (with steam-only choking) is the
        // decay heat before gh:#1041, so the model before it, exactly.
        let od = measure(Mechanisms::default().without_decay_mix().without_capture_correction().without_actinide_decay().without_mixture_choking());
        assert!(near(od.lower_head_h, 4.6678, 0.001), "{od:?}");
        assert!((od.h2_25h_kg - 283.76).abs() < 0.01, "{od:?}");
        assert!(near(od.vent_h, 25.1231, 0.001), "{od:?}");
        // The no-swell ablation. With the ZrO2-melting collapse (in the
        // same change) it differs from the model before gh:#1036 only if a
        // node's cladding reaches 2990 K, which it does not without swell.
        let ns = measure(Mechanisms::default().without_level_swell());
        assert!(near(ns.lower_head_h, 5.371, 0.001), "{ns:?}");
        assert!((ns.h2_25h_kg - 190.59).abs() < 0.01, "{ns:?}");
        assert!((ns.p_dw_peak_mpa - 0.5341).abs() < 0.0001, "{ns:?}");
        // The no-radiation ablation. ~~The model before gh:#1037, exactly
        // (5.3953 h, 216.06 kg, 0.5319 MPa).~~ Since gh:#1036 it carries
        // the swell too, so it isolates radiation.
        let nr = measure(Mechanisms::default().without_core_radiation());
        assert!(near(nr.lower_head_h, 4.973, 0.001), "{nr:?}");
        assert!((nr.h2_25h_kg - 245.33).abs() < 0.01, "{nr:?}");
        assert!((nr.p_dw_peak_mpa - 0.5608).abs() < 0.0001, "{nr:?}");
        // The no-collapse ablation. ~~The model before gh:#1035's collapse
        // step, exactly (5.4125 h, 180.57 kg, 0.5080 MPa).~~ Since
        // gh:#1037 it carries radiation too, so it isolates the collapse.
        let nc = measure(Mechanisms::default().without_rod_collapse());
        assert!(near(nc.lower_head_h, 5.396, 0.001), "{nc:?}");
        assert!((nc.h2_25h_kg - 134.37).abs() < 0.01, "{nc:?}");
        assert!((nc.p_dw_peak_mpa - 0.5094).abs() < 0.0001, "{nc:?}");
        // The no-wall ablation. ~~The model before gh:#1034, exactly (5.399 h,
        // 120.73 kg, 0.4746 MPa).~~ Since Zr's heat of fusion (gh:#1035) it
        // carries that too, so it isolates the wall alone.
        let nw = measure(Mechanisms::default().without_lower_head_wall());
        assert!(near(nw.lower_head_h, 4.331, 0.001), "{nw:?}");
        assert!((nw.h2_25h_kg - 286.97).abs() < 0.01, "{nw:?}");
        assert!((nw.p_dw_peak_mpa - 0.5608).abs() < 0.0001, "{nw:?}");
        // The constant-c_p ablation. ~~The model before gh:#1038, exactly
        // (5.391 h, 126.62 kg, 0.4767 MPa).~~ Since the reaction-heat
        // correction it carries 6.763 MJ/kg too, so it isolates c_p alone.
        // Since gh:#1046: 5.371 -> 5.386 h, 122.96 -> 118.02 kg (H2 at 25 h
        // is not smooth in the inputs: input-sensitivity.md). Since gh:#1034
        // (the wall in both property paths): 5.386 -> 5.535 h, 118.02 ->
        // 116.27 kg. Since rod collapse (gh:#1035): 5.535 -> 5.521 h,
        // 116.27 -> 157.33 kg. Since core radiation (gh:#1037): 5.521 ->
        // 5.653 h, 157.33 -> 135.62 kg.
        let cp = measure(Mechanisms::default().without_temperature_dependent_properties());
        assert!(near(cp.lower_head_h, 5.327, 0.001), "{cp:?}");
        assert!((cp.h2_25h_kg - 162.16).abs() < 0.01, "{cp:?}");
        assert!((cp.p_dw_peak_mpa - 0.5202).abs() < 0.0001, "{cp:?}");
    }

    /// The elasticity of an input that only the demo draws is zero, of one
    /// that sets the lower-head failure is not, and the report ranks them.
    #[test]
    fn elasticities_rank_inputs_by_their_effect() {
        let base = measure(Mechanisms::default());
        let none = elasticity("containment_design_pa", 0.01, &base).unwrap();
        assert_eq!((none.lower_head, none.h2, none.p_dw, none.event_changes), (Some(0.0), 0.0, 0.0, false));
        let lh = elasticity("head_failure_wall_k", 0.01, &base).unwrap();
        assert!(lh.lower_head.is_some_and(|e| e > 0.0), "{lh:?}");
        assert_eq!(lh.tag, Tag::A);
        assert!(elasticity("no_such_input", 0.01, &base).is_none());
        let table = sensitivity_report(&[none, lh]);
        assert!(table.find("head_failure_wall_k").unwrap() < table.find("containment_design_pa").unwrap(), "{table}");
    }

    /// Each ablation runs, differs from the baseline in the mechanism it
    /// switches off and nowhere else, and the report carries every run as a
    /// column with every band as a row.
    #[test]
    fn the_ablation_table_has_every_run_and_band() {
        let runs: Vec<(&str, Metrics)> = ablations().into_iter().map(|(n, mech)| (n, measure(mech))).collect();
        assert_eq!(runs.len(), 12);
        let t = report(&runs);
        for b in BANDS {
            assert!(t.contains(b.name), "{t}");
        }
        for (n, _) in &runs {
            assert!(t.contains(n), "{t}");
        }
        assert_eq!(t.lines().count(), 2 + BANDS.len());
        assert_eq!(ablations()[0].1, Mechanisms::default());
        assert!(!ablations()[1].1.hot_leak && ablations()[1].1.ic_h2_blanketing);
        assert!(ablations()[2].1.hot_leak && !ablations()[2].1.ic_h2_blanketing);
        assert!(!ablations()[3].1.temperature_dependent_properties && ablations()[3].1.hot_leak);
        assert!(!ablations()[4].1.lower_head_wall && ablations()[4].1.temperature_dependent_properties);
        assert!(!ablations()[5].1.rod_collapse && ablations()[5].1.lower_head_wall);
        assert!(!ablations()[6].1.core_radiation && ablations()[6].1.rod_collapse);
        assert!(!ablations()[7].1.level_swell && ablations()[7].1.core_radiation);
        assert!(!ablations()[8].1.decay_mix && ablations()[8].1.capture_correction);
        assert!(!ablations()[9].1.capture_correction && ablations()[9].1.actinide_decay);
        assert!(!ablations()[10].1.actinide_decay && ablations()[10].1.decay_mix);
        assert!(!ablations()[11].1.mixture_choking && ablations()[11].1.actinide_decay);
    }

    /// The verdict reads a value against its band.
    #[test]
    fn verdicts_place_values_against_bands() {
        let b = BANDS[0];
        assert_eq!(verdict(None, &b), "never happens");
        assert_eq!(verdict(Some(5.0), &b), "below");
        assert_eq!(verdict(Some(10.0), &b), "inside");
        assert_eq!(verdict(Some(20.0), &b), "above");
    }
}
