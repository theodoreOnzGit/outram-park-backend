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
    /// Volatile fission products (Xe, Cs, I) released from the fuel by
    /// 25 h, fraction of inventory (gh:#1040). Not a BSAF band: BSAF
    /// reports only that most codes' releases reached an asymptote above
    /// 0.8 (code-to-code, figure-read; NEA No. 7525 §2, Fig. 2.5).
    pub fp_released_25h: f64,
    /// UO2 released from the fuel by 25 h, kg: the seed aerosol (gh:#1047).
    pub uo2_released_25h_kg: f64,
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
            m.fp_released_25h = f.core.fp_volatile_released;
            m.uo2_released_25h_kg = f.core.uo2_released_kg;
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
        ("without gas c_v(T)", all.without_gas_cv_t()),
        ("without diffusion limit", all.without_diffusion_limit()),
        ("without H2 c_p(T)", all.without_hydrogen_cp_t()),
        ("without MSL creep", all.without_msl_creep()),
        ("without dome internals", all.without_dome_internals()),
        ("without penetrations", all.without_penetrations()),
        ("without failed-vessel outflow", all.without_failed_vessel_outflow()),
        ("without pool scrubbing", all.without_pool_scrubbing()),
        ("without in-vessel deposition", all.without_in_vessel_deposition()),
        ("without internals heat loss", all.without_internals_heat_loss()),
        ("without scrubbing saturation", all.without_scrubbing_saturation()),
        ("without swarm scrubbing", all.without_swarm_scrubbing()),
        ("without sectional classes", all.without_sectional_classes()),
        ("without iodine partition", all.without_iodine_partition()),
        ("without mixture vent", all.without_mixture_vent()),
        ("without phoresis", all.without_phoresis()),
        ("without iodine swarm", all.without_iodine_swarm()),
        ("without vessel aerosol", all.without_vessel_aerosol()),
        ("without condensate mass", all.without_condensate_mass()),
        ("without FP decay following", all.without_fp_decay_follows()),
        ("without Te transport", all.without_te_transport()),
        ("without debris oxidation", all.without_debris_oxidation()),
        ("without candling", all.without_candling()),
        ("without debris quench", all.without_debris_quench()),
        ("without wet head contact", all.without_wet_head_contact()),
        ("without penetration coverage", all.without_penetration_coverage()),
        ("without failed-vessel drain", all.without_failed_vessel_drain()),
        ("without wetwell atmosphere", all.without_ww_atmosphere()),
        ("without drywell HS surface", all.without_dw_wall_hs()),
        ("without torus shell", all.without_ww_shell()),
        ("without outflow enthalpy", all.without_outflow_enthalpy()),
        ("without relief H2 enthalpy", all.without_relief_h2_enthalpy()),
        ("without condensate to floor", all.without_condensate_to_floor()),
        ("without RPV wall", all.without_rpv_wall()),
        ("without internals radiation", all.without_internals_radiation()),
        ("without core-top radiation", all.without_core_top_radiation()),
        ("without see-through radiation", all.without_see_through()),
        ("without MCCI", all.without_mcci()),
        ("without drywell outer heat path", all.without_dw_wall_outer()),
        ("without core plate", all.without_core_plate()),
        ("without bottom radiation", all.without_bottom_radiation()),
        ("without ring plates", all.without_ring_plates()),
        ("without pool layers", all.without_pool_layers()),
        ("without plate oxidation", all.without_plate_oxidation()),
        ("without lower-plenum steam consumed", all.without_lp_steam_consumed()),
        ("without control blades", all.without_control_blades()),
        ("without blade fusion", all.without_blade_fusion()),
        ("without cavity phases", all.without_cavity_phases()),
        ("without BUR ignition", all.without_bur_ignition()),
        ("without gas choking", all.without_gas_choking()),
        ("without blade diffusion limit", all.without_blade_diffusion_limit()),
        ("without two-sided equalising", all.without_two_sided_equalising()),
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

/// One member of the perturbation ensemble (2026-10-30, after the fourth
/// sensitivity run found the run chaotic at the 1e-9 level): every input in
/// [`Params::INPUTS`] scaled by `1 + rel u`, `u` uniform in [-1, 1] from a
/// SplitMix64 hash of (`seed`, `member`, the input's index), every
/// mechanism on, then [`measure_with`]. Deterministic: the same arguments
/// give the same run on any machine. `rel = 0` is the default run.
pub fn ensemble_member(member: u64, rel: f64, seed: u64) -> Metrics {
    let mix = |mut z: u64| {
        z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };
    let mut p = Params::default();
    for (i, (name, _)) in Params::INPUTS.iter().enumerate() {
        let h = mix(seed ^ mix(member ^ mix(i as u64)));
        let u = 2.0 * ((h >> 11) as f64 / (1u64 << 53) as f64) - 1.0;
        if let Some(v) = p.get(name) {
            p.set(name, v * (1.0 + rel * u));
        }
    }
    measure_with(Mechanisms::default(), p)
}

/// The ensemble's distribution as Markdown: for each metric, the minimum,
/// the quartiles by the nearest-rank rule, the median and the maximum, and
/// how many members vented and exploded.
pub fn ensemble_report(runs: &[Metrics]) -> String {
    let stats = |mut v: Vec<f64>| -> String {
        if v.is_empty() {
            return "| - | - | - | - | - |".to_string();
        }
        v.sort_by(f64::total_cmp);
        let q = |f: f64| v[((f * v.len() as f64).ceil() as usize).clamp(1, v.len()) - 1];
        format!("| {:.4} | {:.4} | {:.4} | {:.4} | {:.4} |", v[0], q(0.25), q(0.5), q(0.75), v[v.len() - 1])
    };
    let n = runs.len();
    let mut out = format!("{n} members.\n\n| Metric | min | 25 % | median | 75 % | max |\n|---|---|---|---|---|---|\n");
    out.push_str(&format!("| lower-head failure, h {}\n", stats(runs.iter().filter_map(|m| m.lower_head_h).collect())));
    out.push_str(&format!("| depressurised, h {}\n", stats(runs.iter().filter_map(|m| m.depressurised_h).collect())));
    out.push_str(&format!("| in-vessel H2 at 25 h, kg {}\n", stats(runs.iter().map(|m| m.h2_25h_kg).collect())));
    out.push_str(&format!("| drywell peak, MPa {}\n", stats(runs.iter().map(|m| m.p_dw_peak_mpa).collect())));
    out.push_str(&format!("| vent opens, h {}\n", stats(runs.iter().filter_map(|m| m.vent_h).collect())));
    out.push_str(&format!(
        "\nThe lower head failed in {} members, the vent opened in {}, the building exploded in {}.\n",
        runs.iter().filter(|m| m.lower_head_h.is_some()).count(),
        runs.iter().filter(|m| m.vent_h.is_some()).count(),
        runs.iter().filter(|m| m.explosion_h.is_some()).count()
    ));
    out
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

/// The model's histories of the quantities BSAF plots for Unit 1, sampled
/// every `every_s` seconds to `end_s`: time h, RPV pressure MPa, drywell
/// pressure MPa, in-vessel H2 kg, volatile fission products released from
/// fuel (fraction). For comparison with the digitised figures.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Histories {
    pub t_h: Vec<f64>,
    pub p_rpv_mpa: Vec<f64>,
    pub p_dw_mpa: Vec<f64>,
    pub h2_kg: Vec<f64>,
    pub fp_released: Vec<f64>,
    /// Cesium on the drywell's structures and in the wetwell pool,
    /// fractions of the inventory (gh:#1047).
    pub cs_dw_structures: Vec<f64>,
    pub cs_sc_water: Vec<f64>,
    /// Iodine likewise.
    pub i_dw_structures: Vec<f64>,
    pub i_sc_water: Vec<f64>,
    /// Cesium released to the environment (vented to the stack and leaked
    /// to the reactor building), fraction of the inventory.
    pub cs_released: Vec<f64>,
    /// Iodine likewise, and the noble gas (fractions of the core
    /// inventory).
    pub i_released: Vec<f64>,
    pub xe_released: Vec<f64>,
}

pub fn histories(mechanisms: Mechanisms, prm: Params, every_s: f64, end_s: f64) -> Histories {
    let mut plant = Plant::with_params(Mode::Historical, mechanisms, prm);
    let (mut core, mut cont) = plant.parts();
    let mut h = Histories::default();
    let every = (every_s / DT_S).round().max(1.0) as usize;
    let steps = (end_s / DT_S).round() as usize;
    for i in 1..=steps {
        plant.step_inline(&mut core, &mut cont);
        if i % every == 0 {
            let f = plant.frame();
            h.t_h.push(f.t_s / 3600.0);
            h.p_rpv_mpa.push(f.vessel.p_pa / 1e6);
            h.p_dw_mpa.push(f.containment.p_dw_pa / 1e6);
            h.h2_kg.push(f.core.h2_total_kg);
            h.fp_released.push(f.core.fp_volatile_released);
            h.cs_dw_structures.push(f.containment.cs_dw_structures_kg / prm.cs_inventory_kg);
            h.cs_sc_water.push(f.containment.cs_ww_pool_kg / prm.cs_inventory_kg);
            h.i_dw_structures.push(f.containment.i_dw_structures_kg / prm.i_inventory_kg);
            h.i_sc_water.push(f.containment.i_ww_pool_kg / prm.i_inventory_kg);
            h.cs_released.push((f.containment.cs_vented_kg + f.containment.cs_building_kg) / prm.cs_inventory_kg);
            h.i_released.push((f.containment.i_vented_kg + f.containment.i_building_kg) / prm.i_inventory_kg);
            h.xe_released.push(f.containment.xe_vented + f.containment.xe_building);
        }
    }
    h
}

/// Linear interpolation of a sampled series at `t` (None outside it).
fn interpolate(t_s: &[f64], v: &[f64], t: f64) -> Option<f64> {
    if t_s.is_empty() || t < t_s[0] || t > t_s[t_s.len() - 1] {
        return None;
    }
    let i = t_s.partition_point(|&x| x < t);
    if i == 0 {
        return Some(v[0]);
    }
    let w = (t - t_s[i - 1]) / (t_s[i] - t_s[i - 1]);
    Some(v[i - 1] + w * (v[i] - v[i - 1]))
}

/// The model against one measured series (the instrument fixed before any
/// digitised curve was seen, 2026-10-29): the model interpolated at each
/// reference time inside the run, then the mean and the largest absolute
/// difference, in the series' own units, and how many points counted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AgainstMeasured {
    pub n: usize,
    pub mean_abs: f64,
    pub max_abs: f64,
}

pub fn against_measured(t_model: &[f64], v_model: &[f64], reference: &[(f64, f64)]) -> AgainstMeasured {
    let d: Vec<f64> = reference.iter().filter_map(|&(t, r)| interpolate(t_model, v_model, t).map(|m| (m - r).abs())).collect();
    let n = d.len();
    let mean_abs = if n > 0 { d.iter().sum::<f64>() / n as f64 } else { f64::NAN };
    AgainstMeasured { n, mean_abs, max_abs: d.iter().copied().fold(0.0, f64::max) }
}

/// The model against a family of code predictions (no measurement): at
/// each time of `grid_h`, whether the model lies within the codes'
/// envelope (each code interpolated; only codes that cover the time
/// count, and at least two must). Returns the fraction of counted times
/// inside, and how many counted.
pub fn inside_envelope(t_model: &[f64], v_model: &[f64], codes: &[Vec<(f64, f64)>], grid_h: &[f64]) -> (f64, usize) {
    let (mut inside, mut counted) = (0usize, 0usize);
    for &t in grid_h {
        let Some(m) = interpolate(t_model, v_model, t) else { continue };
        let vals: Vec<f64> = codes
            .iter()
            .filter_map(|c| {
                let (ts, vs): (Vec<f64>, Vec<f64>) = c.iter().copied().unzip();
                interpolate(&ts, &vs, t)
            })
            .collect();
        if vals.len() < 2 {
            continue;
        }
        let lo = vals.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = vals.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        counted += 1;
        if m >= lo && m <= hi {
            inside += 1;
        }
    }
    (if counted > 0 { inside as f64 / counted as f64 } else { f64::NAN }, counted)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The comparison instruments on hand-made series: a model `v = t`
    /// against references at t = 0.5, 1.5 (inside) and 5 (outside the run)
    /// differing by 0.1 and 0.3 gives n = 2, mean 0.2, max 0.3; a model
    /// between two codes is inside, one above both is not, and a time
    /// covered by one code alone does not count. The histories sample where
    /// asked.
    #[test]
    fn the_curve_comparisons_measure_what_they_say() {
        let t = [0.0, 1.0, 2.0];
        let v = [0.0, 1.0, 2.0];
        let a = against_measured(&t, &v, &[(0.5, 0.6), (1.5, 1.2), (5.0, 9.0)]);
        assert_eq!(a.n, 2);
        assert!((a.mean_abs - 0.2).abs() < 1e-12 && (a.max_abs - 0.3).abs() < 1e-12, "{a:?}");
        let lo = vec![(0.0, -1.0), (2.0, 1.0)];
        let hi = vec![(0.0, 1.0), (1.0, 3.0)];
        let (frac, n) = inside_envelope(&t, &v, &[lo, hi], &[0.5, 1.0, 1.5]);
        // 0.5: codes -0.5..2, model 0.5 inside; 1.0: codes 0..3, inside;
        // 1.5: only the first code covers it, not counted.
        assert_eq!((frac, n), (1.0, 2));
        let (frac, _) = inside_envelope(&t, &[5.0, 5.0, 5.0], &[vec![(0.0, 0.0), (2.0, 1.0)], vec![(0.0, 1.0), (2.0, 2.0)]], &[1.0]);
        assert_eq!(frac, 0.0);
        let h = histories(Mechanisms::default(), Params::default(), 600.0, 3600.0);
        assert_eq!(h.t_h.len(), 6);
        // The isolation condensers pull the vessel to about 3.7 MPa in the
        // first 10 minutes; stopped at 17 min, it re-pressurises past 7 MPa
        // by 1 h.
        assert!((h.t_h[5] - 1.0).abs() < 1e-12 && h.p_rpv_mpa[0] < 5.0 && h.p_rpv_mpa[5] > 7.0, "{h:?}");
    }

    /// **The uncalibrated baseline, pinned.** Methodology: run the recorded
    /// Unit 1 sequence to 26 h with every mechanism on and measure the six
    /// BSAF quantities. Nothing in the model was tuned toward the bands.
    ///
    /// Results (2026-10-29, measured on branch `claude/digital-twin-engine-vfseuf`
    /// ~~at the physics of `28ff10e31`~~ re-measured at the physics of the
    /// commit adding the internals' heat loss; `--compare` prints the same table):
    ///
    /// | Quantity | Model | BSAF band (tier) | Verdict |
    /// |---|---|---|---|
    /// | lower-head failure | ~~5.39 h~~ ~~5.41 h~~ ~~5.39 h~~ ~~5.40 h~~ ~~5.54 h~~ ~~5.41 h~~ ~~5.40 h~~ ~~5.59 h~~ ~~4.67 h~~ ~~4.51 h~~ ~~4.48 h~~ ~~5.39 h~~ ~~5.45 h~~ ~~5.64 h~~ ~~5.45 h~~ 4.47 h | 7.45-17.16 h (B) | early, regime-fragile |
    /// | RPV depressurised < 2 MPa | ~~5.39 h~~ ~~5.41 h~~ ~~5.39 h~~ ~~5.40 h~~ ~~5.54 h~~ ~~5.41 h~~ ~~5.40 h~~ ~~5.59 h~~ ~~4.67 h~~ ~~4.51 h~~ ~~4.48 h~~ ~~5.39 h~~ ~~4.36 h~~ ~~4.78 h~~ 4.41 h | 5-12 h (A) | ~~inside~~ ~~below,~~ ~~inside, for the wrong reason~~ early: a main steam line ruptures by creep (the BSAF analyses' 3-4 h RCS leak, §1.1), resting on \[A\] inputs |
    /// | in-vessel H2 at 25 h | ~~126.62 kg~~ ~~124.15 kg~~ ~~122.31 kg~~ ~~120.73 kg~~ ~~119.95 kg~~ ~~180.57 kg~~ ~~216.06 kg~~ ~~182.90 kg~~ ~~283.76 kg~~ ~~286.63 kg~~ ~~287.47 kg~~ ~~292.44 kg~~ ~~192.11 kg~~ ~~193.97 kg~~ ~~191.47 kg~~ ~~191.43 kg~~ ~~215.36 kg~~ 295.45 kg | 400-1310 kg (C) | low |
    /// | drywell peak 0-25 h | ~~0.477 MPa~~ ~~0.476 MPa~~ ~~0.475 MPa~~ ~~0.508 MPa~~ ~~0.532 MPa~~ ~~0.510 MPa~~ ~~0.559 MPa~~ ~~0.562 MPa~~ ~~0.564 MPa~~ ~~0.534 MPa~~ ~~0.535 MPa~~ ~~0.547 MPa~~ ~~0.557 MPa~~ 0.595 MPa | 0.8-1.0 MPa (C) | low |
    /// | wetwell vent | ~~never~~ ~~25.12 h~~ ~~23.73 h~~ ~~never~~ 23.73 h | ~24 h (23.5-24.68 h) (A) | ~~missed~~ ~~late~~ ~~inside, regime-fragile~~ ~~missed (regime flip, 1100 K cutoff)~~ inside, regime-fragile (back in the early regime since the internals heat loss) |
    /// | building explosion | never | 24.83 h (A) | missed |
    ///
    /// ~~The RPV depressurises *at* the lower-head failure: the breach lets
    /// the pressure go. Most BSAF analyses have an RCS leak at 3-4 h doing
    /// it (§1.1), so "inside" here is not agreement on the mechanism.~~
    /// **Since 2026-10-29** the RPV depressurises through a main steam line
    /// that fails by creep at ~~4.36 h~~ 4.78 h, the kind of RCS leak most BSAF
    /// analyses have at 3-4 h (§1.1); it is now before the band.
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
    /// **Moved below these pins' precision 2026-10-29** (MELCOR's NCG
    /// `c_v(T)` for the containment's N2 and H2, [`super::gas`], replacing
    /// constant 20.8 / 20.5 J/(mol K)): drywell peak 0.56292285 ->
    /// 0.56293122 MPa (+1.5e-5 relative); lower head, H2 and vent unchanged
    /// to every printed digit. Predicted: drywell peak slightly *down*,
    /// under 1 %. **The sign was wrong**: N2's fitted `c_v` dips to about
    /// 20.5 J/(mol K) near 400 K, below the old constant, so the drywell
    /// gas warms slightly more, not less. The magnitude was right: steam
    /// carries the drywell's heat capacity. `without_gas_cv_t` reproduces
    /// the previous fixture byte for byte (checked by running it as the
    /// default), and both drywell values are pinned below.
    ///
    /// **Moved again 2026-10-29** (MELCOR's gaseous-diffusion limit on Zr
    /// oxidation, COR Eq 2-168, [`super::core::diffusion_limited_rate`]):
    /// lower head 4.5089 -> 4.4844 h, H2 287.47 -> 292.44 kg, drywell
    /// 0.56293 -> 0.56407 MPa, vent unchanged. Predicted: binds only near
    /// steam starvation, H2 slightly *down* (under 1 %), the rest about
    /// unchanged. **Where it binds was right** (555 node-steps in 30 h, all
    /// with the steam mole fraction under about 3 %); **the sign on H2 was
    /// wrong**: the steam a starved node no longer takes passes up the ring,
    /// and the small redistribution moves the run's timing (the regime
    /// fragility above; H2 at 25 h is not smooth in the inputs).
    /// `without_diffusion_limit` reproduces the previous baseline exactly
    /// (pinned below).
    ///
    /// **Moved again 2026-10-29, by a regime flip** (oxidation cutoff
    /// `oxidation_min_k` 1000 K \[A\] -> MELCOR's TOXMIN 1100 K \[P\], also
    /// the low end of the Urbanic-Heidrick fit's range): lower head 4.4844
    /// -> 5.390 h, H2 292.44 -> 192.11 kg, drywell 0.56407 -> 0.53414 MPa,
    /// **no vent** (was 23.73 h, inside its Tier A band; now it is not
    /// reached). Predicted: H2 down under 1 %, the rest about unchanged
    /// unless the regime flipped. **It flipped.** A diagnostic scan (not a
    /// tuning run: the value is MELCOR's, chosen before the scan) shows the
    /// outcome is bimodal in the cutoff, with flips both ways: 1000-1025 K
    /// early regime (4.48-4.51 h, 287-292 kg), 1050-1125 K late (5.35-5.40
    /// h, 188-193 kg), 1150 K early again (4.53 h, 279 kg). So this input,
    /// which oxidises little directly, acts only through which regime the
    /// run lands in. No lower-head or H2 comparison is meaningful until the
    /// slug's size stops deciding the run (core-plate holdup, F4, blocked
    /// on L4). The previous baseline is reproduced exactly with
    /// `oxidation_min_k` = 1000 K (pinned below, `old`).
    ///
    /// **Moved again 2026-10-29** (hydrogen's `c_p(T)` in the core and the
    /// vessel, MELCOR's NCG fit, [`super::gas`]; the constant was 20.5 + R
    /// J/(mol K), the fit reaches about 34 J/(mol K) at 2000 K): lower head
    /// 5.3900 -> 5.3925 h, H2 192.11 -> 193.97 kg (+1.0 %), drywell 0.53414
    /// -> 0.53475 MPa. Predicted: H2 down a few percent, lower head later.
    /// **Lower head held** (later, but by 0.0025 h); **the H2 sign missed**.
    /// ~~Not traced; one candidate is that a cooler stream delays rod
    /// collapse and keeps Zr in the oxidising core longer.~~ **Traced the
    /// same day, candidate refuted:** the two fixtures relocate identically
    /// at every 10-minute sample; the 1.9 kg builds up in the 4.0-4.5 h
    /// burst, during which the new run's peak clad is *hotter* (2508
    /// against 2460 K at 4.33 h), not cooler. A first-order argument for
    /// the sign of H2 at 25 h does not survive the burst's dynamics; three
    /// sign misses in a row (diffusion limit, cutoff, this) say the same.
    /// `without_hydrogen_cp_t`
    /// reproduces the previous baseline exactly (pinned below).
    ///
    /// **Moved again 2026-10-29** (creep rupture of a main steam line,
    /// [`super::msl`], MELCOR's PIPE-STR and LM-CREEP with A-508's
    /// coefficients, the wall heated by the relief flow at the core's exit
    /// gas temperature): a line ruptures at 4.36 h, during the 4.17-4.33 h
    /// burst (wall 1046-1090 K at 8 MPa), and the vessel depressurises
    /// through it. RPV below 2 MPa 5.3925 -> 4.358 h, lower head 5.3925 ->
    /// 5.4544 h, H2 193.97 -> 191.47 kg, drywell 0.53475 -> 0.54734 MPa.
    /// Predicted before the run: rupture at 4.2-4.4 h if the relief duty
    /// heats the wall past about 1050 K, the vessel depressurising at it,
    /// the drywell peak up "substantially". **Held** on the rupture time
    /// and the depressurisation; the drywell rose 2.4 %, less than
    /// "substantially". ~~Every one-mechanism ablation also ruptures
    /// (3.68-4.36 h), including the one without the \[A\] hot leak (4.28
    /// h): the physics now supplies the early RPV leak the BSAF analyses
    /// needed (§1.1).~~ **CORRECTED the same day**: that held only with the
    /// gas reaching the line at the core's exit temperature. With the dome
    /// internals (below) four of the five one-mechanism ablations do not
    /// rupture, so the early leak is **not** robust. The depressurisation is now *before* its BSAF band
    /// (5-12 h). `without_msl_creep` reproduces the previous baseline
    /// exactly (pinned below).
    ///
    /// **Corrected 2026-10-29** (a defect, not physics: a dry vessel's
    /// outflow is now capped at the isothermal equalising amount, the
    /// containment's rule for stiff paths; without it a large break could
    /// empty the vessel in one step, found when a perturbed input sent the
    /// pressure to infinity): lower head 5.4544 -> 5.4492 h, H2 191.47 ->
    /// 191.43 kg, drywell 0.54734 -> 0.54685 MPa, depressurisation
    /// unchanged. Runs without a steam-line rupture never bind the cap and
    /// do not move.
    ///
    /// **Corrected again 2026-10-29** (a second defect: steam leaving a dry
    /// vessel left at saturated-vapour enthalpy, leaving its superheat
    /// behind, so a draining vessel's specific energy grew without bound;
    /// now it leaves at `u/m + p V/m`, `Plant::steam_out_h`): the baseline
    /// moves only in its drywell peak, 0.54685193 -> 0.54684943 MPa. Every
    /// run whose vessel boils dry is touched, so **the "previous baseline,
    /// exactly" ablations below now reproduce the earlier models with this
    /// fix applied, not their historical numbers**: each pin below carries
    /// its old and new value (lower head by at most 0.012 h, H2 by at most
    /// 1.4 kg).
    ///
    /// **Moved again 2026-10-29** (the steam dome's internals as one heat
    /// structure, [`super::internals`]: 50 t, 2500 m^2, 10 W/(m^2 K), all
    /// \[A\]; the gas reaching the steam lines, the leaks and the
    /// containment is cooled by them): the steam line still ruptures, but
    /// at 4.78 h, not 4.36; lower head 5.4492 -> 5.6392 h, H2 191.43 ->
    /// 215.36 kg (+12.5 %), drywell 0.54685 -> 0.55625 MPa. Predicted
    /// before the run: NTU about 3, so **no rupture** before the lower
    /// head. **Missed**: the internals warm (to about 840 K) and the gas
    /// they pass stays hot enough. A diagnostic scan of the \[A\] area (not
    /// tuning; the default was fixed first): 0.03x and 0.1x rupture at
    /// about 4.3 h and flip to the early regime (lower head 4.74-4.81 h, H2
    /// 297-318 kg); 0.3x ruptures at 4.36 h; 1x at 4.78 h; 3x does not
    /// rupture (depressurised at the lower head, 5.71 h). Of the
    /// one-mechanism ablations only no-radiation still ruptures (4.34 h).
    /// So **whether and when the line fails rests on \[A\] inputs**, the
    /// internals' area and mass among them: a first-order finding for
    /// F13. `without_dome_internals` reproduces the previous baseline
    /// exactly (pinned below).
    ///
    /// **Moved again 2026-10-29** (lower-head penetrations: one lumped
    /// node of 10 mm steel \[A\], heated by the debris at 1000 W/(m^2 K)
    /// and failing the head at TPFAIL 1273.15 K, as MELCOR's §6.3 fails on
    /// "a penetration or the innermost head node"): lower head 5.6392 ->
    /// 5.4517 h, H2 unchanged, drywell 0.55625 -> 0.55679 MPa. Predicted:
    /// the penetration's time constant is about 40 s, so failure tracks
    /// the dry debris crossing about 1273 K, 0.1-0.3 h earlier. **Held**
    /// (0.19 h). Insensitive to the \[A\] thickness: 3 mm 5.442 h, 30 mm
    /// 5.479 h. `without_penetrations` reproduces the previous baseline
    /// exactly (pinned below).
    ///
    /// **Moved again 2026-10-29** (a failed vessel's gas leaves through the
    /// breach on its own pressure; before, nothing left it): drywell peak
    /// 0.55678708 -> 0.55680172 MPa, the rest unchanged. Predicted: +0.01
    /// MPa from about 20 kg of hydrogen made after the failure. **Missed**:
    /// almost no hydrogen is made after the failure (the containment holds
    /// 214.8 of the 215.4 kg by 6 h), so only the vessel's late steam moves,
    /// and it condenses; the "about 20 kg" was a misreading against an
    /// older fixture. `without_failed_vessel_outflow` reproduces the
    /// previous baseline exactly (pinned below).
    ///
    /// **Moved again 2026-10-29, by a regime flip** (the dome internals
    /// lose heat to the saturated dome by natural convection while the
    /// vessel holds water, `internals_heat_loss`, 5 W/(m^2 K) \[A\]; before,
    /// nothing cooled them once the flow stopped): lower head 5.4517 ->
    /// 4.4692 h, depressurised 4.7769 -> 4.4061 h, H2 215.36 -> 295.45 kg,
    /// drywell 0.55680 -> 0.59493 MPa, **the vent flows again** (23.73 h).
    /// Predicted before the run: the internals cool, the gas reaching the
    /// steam lines runs cooler, the line ruptures later or not at all.
    /// **Missed**: the run flipped into the early regime and depressurises
    /// earlier. Found on the way: in the no-radiation ablation the vessel
    /// ~~empties (its mass reaches 0 before the head fails, oxidation still
    /// drawing steam from it), and the heat loss then sent its pressure to
    /// 227 MPa; the loss is now limited to a wet vessel, and the emptying is
    /// filed as its own defect~~ **CORRECTED the same day**: the emptying
    /// was caused by the heat loss itself (into a dry vessel, its heat kept
    /// the pressure above the drywell's and the gas drained out through the
    /// leak; oxidation drew nothing); with the heat loss off the vessel
    /// never goes below 180 kg. Limiting the loss to a wet vessel removes
    /// it (gh:#1051). `without_internals_heat_loss` reproduces the
    /// previous baseline exactly (pinned below).
    ///
    /// **Moved again 2026-10-29** (the decay heat of the released noble
    /// gases, alkali metals and halogens leaves the core with them,
    /// `fp_decay_follows`, the class shares from `fp_decay`'s ENDF/B-VIII
    /// summation; the vessel's share onto the dome internals, which hold its
    /// deposit): lower head 4.4692 -> 4.5239 h, depressurised 4.4061 ->
    /// 4.4361 h, H2 295.45 -> 304.26 kg, drywell peak 0.59493 -> 0.61280
    /// MPa, UO2 released 2.634 -> 3.327 kg, vent unchanged. Predicted: lower
    /// head 0-0.3 h later (held: 0.055 h); the drywell cooler late (the
    /// peak, before the vent, rose instead: the heat reaches the
    /// containment with the fission products). No sign was predicted for
    /// H2. `without_fp_decay_follows` reproduces the previous baseline
    /// exactly (pinned below).
    ///
    /// **Moved again 2026-10-29** (tellurium transported, MELCOR's chalcogen
    /// class, 15.4 kg \[A\] from the summation's Te/Cs ratio, its decay heat
    /// following it, `te_transport`): lower head 4.5239 -> 4.5172 h,
    /// depressurised 4.4361 -> 4.4497 h, H2 304.26 -> 305.64 kg, drywell
    /// peak 0.61280 -> 0.60869 MPa, release 0.7967 -> 0.7977, UO2 3.327 ->
    /// 3.645 kg. **No prediction was written before this run** (a lapse of
    /// the rule, recorded): taking more heat out of the core would be
    /// expected to delay the lower head, and it came 0.007 h earlier,
    /// within the run's regime noise. `without_te_transport` reproduces the
    /// previous baseline exactly (pinned below).
    ///
    /// **Moved again 2026-10-29** (the lower-plenum debris oxidises while
    /// quenched, MELCOR's particulate area `6 V / D` with D = 1 cm \[A\],
    /// the steam it raises as its supply, `debris_oxidation`): lower head
    /// 4.5172 -> 4.4831 h, depressurised 4.4497 -> 4.4489 h, H2 305.64 ->
    /// 313.43 kg, drywell peak 0.60869 -> 0.62303 MPa, release 0.7977 ->
    /// 0.8109, UO2 3.645 -> 3.516 kg. Predicted: H2 +10-40 % (**missed**:
    /// +2.5 %), lower head 0-0.3 h earlier (held: 0.034 h). A diagnostic of
    /// the \[A\] diameter (not tuning; 1 cm was fixed first): 0.5 cm gives H2
    /// 245.55 kg, 2 cm 305.27 kg. Non-monotonic in the area, so H2 at 25 h
    /// here is carried by the run's timing (dryout, failure), not by the
    /// debris chemistry. `without_debris_oxidation` reproduces the previous
    /// baseline exactly (pinned below).
    ///
    /// **Moved again 2026-10-29, by much** (candling: once the oxide shell
    /// breaches, molten Zr flows down the rod and refreezes in the first
    /// node below Zr's melting point, or reaches the debris, `candling`):
    /// lower head 4.4831 -> 5.3144 h, depressurised 4.4489 -> 4.5206 h, H2
    /// 313.43 -> 441.97 kg (+41 %), drywell peak 0.62303 -> 0.70451 MPa,
    /// release 0.8109 -> 0.9911, UO2 3.516 -> 2.780 kg, vent unchanged.
    /// Predicted: H2 +0-20 % and the lower head within 0.2 h. **Both
    /// missed in size**: the refrozen metal lower down oxidises when those
    /// nodes heat, and their larger heat capacity delays the melt. H2 is now
    /// inside its BSAF band (398-1315 kg) for the first time; the lower
    /// head (band 7.45-17.16 h) and the drywell peak (0.8-1.0 MPa) moved
    /// toward theirs. Zr is conserved through it to 1e-9
    /// (`core::tests::zircaloy_is_conserved_through_candling`).
    /// `without_candling` reproduces the previous baseline exactly (pinned
    /// below).
    ///
    /// **Moved again 2026-10-29** (MELCOR's falling-debris quench, COR
    /// §2.3.8: the decay heat stays in the debris, which gives the water
    /// `h f A_p (T - T_sat)` at 100 W/(m^2 K) over the particle area with
    /// the 360 s decay factor, then Lipinski's dryout limit; replacing the
    /// 120 s \[A\] time constant, `debris_quench`): lower head 5.3144 ->
    /// 4.7364 h, depressurised 4.5206 -> 4.6206 h, H2 441.97 -> 395.54 kg
    /// (just below its band's 398), drywell peak 0.70451 -> 0.72921 MPa,
    /// release 0.9911 -> 0.9978, UO2 2.780 -> 3.949 kg. Predicted the lower
    /// head within 0.1 h (an estimate that the bed is coolable and the
    /// quench fast): **missed**, 0.58 h earlier. While the decay factor is
    /// between 1 and 0.01 the debris keeps its decay heat and heats under
    /// water; the plenum then dries onto hotter debris. In the old model
    /// the decay heat went straight to the water. `without_debris_quench`
    /// reproduces the previous baseline exactly (pinned below).
    ///
    /// **Moved again 2026-10-29** (the lower head takes heat from the debris
    /// that touches it under water too, as MELCOR's head conducts from the
    /// debris in contact, `wet_head_contact`; and the penetrations take it
    /// only over the part of their height the debris covers, `dz_d /
    /// dz_1` with a 0.5 m bottom level \[A\], the rest cooled by the water,
    /// `penetration_coverage`, COR §6.1): lower head 4.7364 -> 4.5558 h,
    /// now the depressurisation too; H2 395.54 -> 253.13 kg, drywell peak
    /// 0.72921 -> 0.58922 MPa, release 0.9978 -> 0.9955, UO2 3.949 ->
    /// 4.891 kg. Predicted: the head earlier, by up to 0.3 h. **Held**
    /// (0.18 h); no H2 prediction was written. Wet contact without the
    /// coverage fails the head at the first debris (3.554 h, 89 kg H2, no
    /// vent; pinned). **Caveat:** with no core-plate holdup (blocked on
    /// L4), the first small relocations reach the head directly, so this
    /// head time leans on a missing mechanism. `without_wet_head_contact`
    /// reproduces the previous baseline exactly (pinned below).
    ///
    /// **Moved again 2026-10-29** (a failed vessel's water drains through
    /// the breach, 0.1 m \[A\], into the pedestal, `failed_vessel_drain`;
    /// before, the 17-21 t left in the plenum at a wet failure stayed in
    /// the vessel until it boiled away): drywell peak 0.58922 -> 0.58863
    /// MPa; lower head, H2, vent and release unchanged. Predicted the
    /// drywell peak up 0.02-0.1 MPa (more water on the debris): **missed**,
    /// it moved down 0.0006 MPa; predicted in-vessel H2 down by under 20
    /// kg: held (no change). Without the drain the no-radiation ablation
    /// went NaN once the plenum dried (a node candled bare: 0/0 in the clad
    /// inversion, fixed in `core.rs`). `without_failed_vessel_drain`
    /// reproduces the previous baseline exactly (pinned below).
    ///
    /// **Moved again 2026-10-29** (the wetwell atmosphere has its own
    /// temperature and steam, `ww_atmosphere`: MELCOR's pool-surface
    /// exchange, CVH/FL RM §5.1, and bubble model at the vents, §6.1):
    /// drywell peak 0.58863 -> 0.56194 MPa; lower head, H2, vent and release
    /// unchanged. Predicted the peak within 0.01 MPa: **missed** (-0.027).
    /// After about 8 h no gas bubbles through the pool, so steam reaches the
    /// wetwell gas only by evaporation at its surface, which lags the pool's
    /// warming: at 22 h the gas is 1.1 K below the pool at a relative
    /// humidity of about 0.87, and the wetwell is 22 kPa lower (0.5288
    /// against 0.5512 MPa). A hand estimate of the surface's mass transfer
    /// at 22 h (MELCOR's analogy, `C` about 2.8e-3 kg/(m^2 s), energy
    /// limited to about 0.03 kg/s) agrees with the run's 120 kg/h. The
    /// torus shell is still not a heat structure (it would condense more).
    /// `without_ww_atmosphere` reproduces the previous baseline exactly
    /// (pinned below).
    ///
    /// **Moved again 2026-10-29** (the drywell structures as MELCOR's
    /// heat-structure surface, `dw_wall_hs`: natural convection, HS RM
    /// Table 2.3, and condensation by diffusion, Eq 2-136, its latent heat
    /// into the wall, in place of a fixed 20 W/(m^2 K) \[A\]): drywell peak
    /// 0.56194 -> 0.56080 MPa, vent 23.734 -> 23.887 h; lower head, H2 and
    /// release unchanged. Predicted the peak +0.02 MPa (range -0.02 to
    /// +0.06): inside the range, **sign missed**. From 3.5 to 6 h the walls,
    /// below the dew point, condense 1-3 kg/s and heat faster (407 against
    /// 375 K at 5 h), so the drywell is lower then (0.354 against 0.367
    /// MPa); after about 10 h the walls are hotter than the gas (the dry
    /// pedestal's debris heat goes into them) and the computed coefficient,
    /// about 5 W/(m^2 K), returns less of it, so the drywell is 3 kPa lower
    /// before the vent, which opens later. `without_dw_wall_hs` reproduces
    /// the previous baseline exactly (pinned below).
    ///
    /// **Moved again 2026-10-29** (the torus shell above the pool as a heat
    /// structure on the wetwell atmosphere, `ww_shell`; 1353 m^2 and
    /// 7.96e7 J/K \[A\], outer face adiabatic): vent 23.887 -> 24.013 h;
    /// drywell peak 0.56079836 -> 0.56079765 MPa, the rest unchanged.
    /// Predicted the peak down 0-0.005 MPa (held) and the vent within 0.1 h
    /// later (**missed**, 0.13 h). The peak is set by the rupture disk: the
    /// drywell peaks just before the vent opens, at the disk's pressure plus
    /// the submergence head, so a lower wetwell delays the vent rather than
    /// lowering the peak. `without_ww_shell` reproduces the previous
    /// baseline exactly (pinned below).
    ///
    /// **Moved again 2026-10-29** (gas leaving the drywell through the
    /// downcomers and the flange leak takes its enthalpy, MELCOR's donor
    /// convention, CVH/FL RM Eq 6-6, `outflow_enthalpy`): drywell peak
    /// 0.56080 -> 0.56641 MPa, vent 24.013 -> 23.734 h (back to the order's
    /// time); lower head, H2 and release unchanged. Predicted the peak
    /// within 0.002 MPa and the vent 0-0.1 h later: **both missed**. After
    /// the head fails the downcomers push hot gas continuously, and each
    /// mole now carries `R T` more (about 40 % more energy at 700 K) into
    /// the pool: 2 K warmer at 23.7 h, the wetwell higher, past the rupture
    /// disk's pressure by the order's time, so the vent opens at the order;
    /// the drywell gas is 43 K cooler (696 against 739 K) but higher, a
    /// submergence head above the wetwell. (So the torus shell's note above
    /// holds only when the disk, not the order, sets the vent.)
    /// `without_outflow_enthalpy` reproduces the previous baseline exactly
    /// (pinned below).
    ///
    /// **Moved again 2026-10-29** (the relief valves' hydrogen reaches the
    /// wetwell with its enthalpy at the vessel's saturation temperature,
    /// closing what the vessel gives up, `relief_h2_enthalpy`): drywell peak
    /// 0.56641 -> 0.56659 MPa, the rest unchanged. Predicted +0 to +0.002
    /// MPa (about 0.9 GJ more into the pool, +0.1 K): **held**.
    /// `without_relief_h2_enthalpy` reproduces the previous baseline exactly
    /// (pinned below).
    ///
    /// **Moved again 2026-10-29** (water condensed in the drywell drains to
    /// its floor, the pedestal's pool, instead of leaving the model,
    /// `condensate_to_floor`): drywell peak 0.56659 -> 0.58660 MPa, the rest
    /// unchanged. Predicted +0.02 MPa (range 0 to +0.06): **held**. 12 t
    /// is on the floor when the head fails (4.56 h); the debris boils it
    /// and what condenses returns, so the pedestal dries about 2 h later,
    /// the walls run 60 K cooler (666 against 727 K at 22 h) and the
    /// drywell holds more steam, 19 kPa higher at 22 h.
    /// `without_condensate_to_floor` reproduces the previous baseline
    /// exactly (pinned below).
    ///
    /// **Moved again 2026-10-29** (the RPV wall, cylinder and top head,
    /// 0.16 m of steel \[A\], about 424 t, as a lumped heat structure on the
    /// vessel's saturated contents, with 1 W/(m^2 K) insulation to the
    /// drywell, `rpv_wall`): lower head 4.5558 -> 4.6236 h, H2 253.13 ->
    /// 228.61 kg, drywell peak 0.58660 -> 0.57524 MPa, release 0.9955 ->
    /// 0.9961, UO2 4.891 -> 5.563 kg; vent unchanged. Predicted the head
    /// within 0.01 h, H2 within 2 kg and the peak +0.01 MPa: **all missed**.
    /// The premise, that the wall sits at saturation before the head fails,
    /// was wrong: the isolation condenser's cycling swings the vessel
    /// between 3.5 and 7.9 MPa in the first hour, and the wall buffers it
    /// (the vessel falls to 3.46 MPa, not 2.88), then absorbs about 1.5 GJ
    /// as it repressurises, so less water boils (128 against 126 t at
    /// 1.25 h); uncovery and the late depressurisation come later and the
    /// head fails 0.07 h later. H2 at 25 h is set by the time from
    /// uncovery to slump (see the input-sensitivity record).
    /// `without_rpv_wall` reproduces the previous baseline exactly (pinned
    /// below).
    ///
    /// **Moved again 2026-10-29** (the dome internals radiate to the RPV
    /// wall, MELCOR's gray pair, HS RM Eq 2-96, with its default steel
    /// emissivity, COR C1102, over a 78.4 m^2 envelope \[A\],
    /// `internals_radiation`): lower head 4.6236 -> 4.6225 h, H2 228.61 ->
    /// 227.76 kg, drywell peak 0.57524 -> 0.57442 MPa, UO2 5.563 -> 5.556
    /// kg. Predicted the head within 0.02 h, H2 within 5 kg, the peak within
    /// 0.005 MPa: **held**; the internals below 800 K at 26 h: **missed**
    /// (812 K, 1332 K without it); more cesium held on them: **missed**
    /// (19.40 against 19.45 kg: neither re-evaporates by 26 h now).
    /// `without_internals_radiation` reproduces the previous baseline
    /// exactly (pinned below). (Measured first with a duplicate steel
    /// emissivity at 616.4833 K; on the model's one function, at 616.48 K,
    /// the same day: lower head 4.6225 -> 4.6231 h, H2 227.76 -> 227.86 kg,
    /// peak 0.57442 -> 0.57458 MPa, UO2 5.556 -> 5.558 kg. A 1.2e-6 change
    /// in emissivity moves the slump timing by 0.0006 h.)
    ///
    /// **Moved again 2026-10-29** (the top of the core radiates to the dome
    /// internals, its axial boundary structure, MELCOR COR RM §2.1.2 with
    /// `Fcell,a` = `axial_exchange`, through emptied cells,
    /// `core_top_radiation`): lower head 4.6231 -> 4.6228 h, H2 227.86 ->
    /// 226.81 kg, drywell peak 0.57458 -> 0.56772 MPa, UO2 5.558 -> 6.171
    /// kg. Predicted the head within 0.03 h and H2 within 10 kg (**held**),
    /// the peak within 0.005 MPa (**missed**, -0.0069). After the slump
    /// the fuel still standing radiates about 250 kW of its decay heat up
    /// to the internals (931 against 813 K at 23.7 h) and through them to
    /// the wall and its insulation; without the path those nodes heated
    /// until they fell and their heat left with the debris. Less reaches
    /// the containment (pool 1.2 K cooler at 23.7 h), and the hot standing
    /// fuel releases more UO2. `without_core_top_radiation` reproduces the
    /// previous baseline exactly (pinned below).
    ///
    /// **Moved again 2026-10-29** (the core's radiation looks through
    /// emptied cells to the next standing component, radially or to the
    /// shroud and axially, MELCOR's COR RM §2.1.2, `see_through`): lower head
    /// 4.6228 -> 4.6231 h, H2 226.81 -> 226.85 kg, drywell peak 0.56772 ->
    /// 0.56766 MPa, UO2 6.171 -> 6.598 kg. Predicted the head within 0.05 h,
    /// H2 within 10 kg, the peak within 0.005 MPa: **held**; no UO2
    /// prediction written (standing fuel beside emptied cells now trades
    /// heat with the neighbours it sees). `without_see_through` reproduces
    /// the previous baseline exactly (pinned below).
    ///
    /// **Moved again 2026-10-29** (molten core-concrete interaction on the
    /// drywell floor, a reduced MELCOR CAV, `mcci`: CORCON basaltic concrete,
    /// quasi-steady ablation with h = 1000 W/(m^2 K) over 21.2 m^2 \[A\],
    /// the concrete's H2O and CO2 reduced by the debris's Zr, film boiling
    /// under water, radiation when dry): drywell peak 0.56766 -> **0.77351
    /// MPa**; the flange leak opens (about 23 h); lower head, in-vessel H2,
    /// vent and release unchanged. Predicted the peak +0.05 to +0.2 MPa:
    /// **held** (+0.206, at the edge). By 25 h 41.4 t of concrete is
    /// ablated (about 0.8 m deep), giving 271 kg of H2 and 395 kg of CO;
    /// most of it while the floor is dry, 9.5-16 h; the debris then cools
    /// under the returning water to below the ablation temperature. The
    /// predicted 100-500 kg of extra H2 held (271). `without_mcci`
    /// reproduces the previous baseline exactly (pinned below).
    ///
    /// **Moved again 2026-10-29** (the drywell structures' outer heat path:
    /// across the shell-to-concrete gap by gas conduction and gray
    /// radiation into a 1-D, 1.5 m concrete slab to the reactor building,
    /// `dw_wall_outer`, all \[A\]): drywell peak 0.77351 -> 0.68843 MPa, the
    /// flange leak no longer opens; lower head 4.6231 -> 4.6225 h, H2 226.85
    /// -> 226.71 kg. Predicted the peak down 0.03-0.10 MPa: **held**
    /// (-0.085). `without_dw_wall_outer` reproduces the previous baseline
    /// exactly (pinned below).
    ///
    /// **Moved again 2026-10-29** (relocating material rests on the BWR core
    /// plate until it yields, MELCOR's PLATEB, COR RM §5.2.3 and Eq 5-16,
    /// 0.05 m on 0.3 m beams \[A\], `core_plate`): lower head 4.6225 ->
    /// 5.0003 h, H2 226.71 -> 183.59 kg, drywell peak 0.68843 -> 0.64074
    /// MPa, release 0.9961 -> 0.9965, UO2 6.598 -> 6.487 kg. Predicted the
    /// head later by up to 0.3 h, H2 within 20 kg, the peak within 0.02
    /// MPa: **all missed**. The first 2.4 t lands on the plate at 3.6 h, as
    /// the level falls below it; held dry, it and the plate heat on decay
    /// heat alone (1187 K by 4.67 h); a hotter second batch takes the plate
    /// to 1696 K and it fails at 4.99 h, dropping 13 t onto the head, which
    /// (in wet contact) fails at 5.00 h. The H2 falls because the first
    /// debris no longer quenches in the lower plenum's water, whose steam
    /// fed the core's oxidation above: the core is steam-starved from 3.67 h
    /// (H2 stays at 75 kg) until the plate fails. The bottom of the core's
    /// radiation to that water, deferred as small, is the missing steam
    /// source this exposes. `without_core_plate` reproduces the previous
    /// baseline exactly (pinned below).
    ///
    /// 2026-10-29, the bottom of the core radiating to the lower plenum's
    /// water (MELCOR COR RM §2.1.2, `FLPUP` 0.25, water emissivity 0.95
    /// \[A\], `bottom_radiation`): lower head 5.0003 -> 5.9828 h,
    /// depressurised 5.0003 -> 5.2419 h, H2 at 25 h 183.59 -> 296.36 kg,
    /// drywell peak 0.64073627 -> 0.56079567 MPa, UO2 released 6.487 ->
    /// 2.847 kg. Predicted (written first): H2 up 20-80 kg, the head
    /// failing 0.05-0.3 h EARLIER, the peak within 0.03 MPa; all three
    /// missed. The mechanism itself is small (about 0.2 MW from the whole
    /// core at 1500 K) and does nothing before the plate fails, since debris
    /// on the plate blocks the view. What moved the run is that the head's
    /// failure at 5.00 h is a knife edge: the two runs agree to 0.1 MPa and
    /// a few kelvin up to 18000 s, the old one's head fails at 18005 s
    /// (943 K, 8.16 MPa) and the new one's, at 915 K while the relief valves
    /// cycle the pressure, does not; the debris then quenches in the water,
    /// its steam oxidises the core (+113 kg H2), the hot gas creeps the main
    /// steam line through at 5.24 h, and the head fails dry at 5.98 h. The
    /// head-failure time is therefore not resolved to better than about 1 h
    /// by this model; the H2 and the drywell peak follow from which branch
    /// is taken. `without_bottom_radiation` reproduces the previous baseline
    /// exactly (pinned below).
    ///
    /// 2026-10-29, one core plate per radial ring (MELCOR COR RM §5.2.3,
    /// `ring_plates`): lower head 5.9828 -> 4.5900 h, depressurised 5.2419
    /// -> 4.5900 h, H2 296.36 -> 253.50 kg, drywell peak 0.56079567 ->
    /// 0.75497232 MPa, vent 24.1631 -> 23.7336 h, UO2 2.847 -> 4.647 kg.
    /// Predicted (written first): the first plate failing 0.1-0.5 h earlier,
    /// the run staying on the late branch (head 5.5-6.5 h), H2 +0-60 kg,
    /// the peak within 0.05 MPa. The first held 2.4 t sits on ring 1's
    /// plate, a third of the area, so three times the load per area: it
    /// fails at 4.47 h (0.52 h earlier, just outside the prediction), and
    /// that smaller slug fails the head in wet contact at 4.59 h, the early
    /// branch, which the prediction ruled out; the other plates fail at 4.90
    /// and 5.42 h. The branch flip carries the rest. `without_ring_plates`
    /// reproduces the previous baseline exactly (pinned below).
    ///
    /// 2026-10-29, the torus pool in two layers split at the downcomers'
    /// exits (1.21 m, `pool_layers`; PPOOLEX, Li and Kudinov): drywell peak
    /// 0.75497232 -> 0.96464924 MPa, H2 253.50 -> 256.41 kg, head 4.5900 ->
    /// 4.5914 h, UO2 4.647 -> 4.525 kg, vent unchanged at 23.7336 h.
    /// Predicted (written first): nothing before the head fails (held: the
    /// relief valves heat the deep layer, which mixes); the surface layer at
    /// 380-400 K by 11.7 h (378 K at 12 h, the mixed pool 356 K); the
    /// drywell up 0.1-0.2 MPa at 11.7 h (+0.084 at 12 h, low); the peak
    /// 0.80-0.95 MPa (0.965, just above); the flange open before 20 h (held,
    /// about 17.5 h) and the building's H2 below 4 % (held, 1.9 % at 24 h);
    /// the in-vessel metrics unchanged to 0.01 (the head held, H2 moved 2.9
    /// kg: missed). `without_pool_layers` reproduces the previous baseline
    /// exactly (pinned below).
    ///
    /// 2026-10-29, the debris on the core plates oxidising in the steam
    /// rising past them (`plate_oxidation`): H2 256.41 -> 252.87 kg, drywell
    /// peak 0.96464924 -> 0.96523341 MPa, UO2 4.525 -> 4.630 kg, the head
    /// unchanged at 4.5914 h. Predicted (written first): H2 +0-15 kg (the
    /// sign missed), the head within 0.1 h and the peak within 0.02 MPa
    /// (both held). The likely reason for the sign: the core above is
    /// steam-starved, so the steam the plate takes is steam the core would
    /// have turned into hydrogen anyway; the hydrogen is made lower, not in
    /// a greater amount (not separately measured). `without_plate_oxidation`
    /// reproduces the previous baseline exactly (pinned below).
    ///
    /// 2026-10-29, a conservation defect fixed (`lp_steam_consumed`): the
    /// lower plenum's debris oxidised with the steam it raised and the core
    /// could oxidise with the same steam again, since all of the plenum's
    /// boil-off rose into it. H2 252.87 -> 247.99 kg, drywell peak
    /// 0.96523341 -> 0.96498045 MPa, UO2 4.630 -> 5.052 kg, the head
    /// unchanged. Predicted (written first): H2 down 0-10 kg, the head within
    /// 0.02 h, the peak within 0.01 MPa; all held. `without_lp_steam_consumed`
    /// reproduces the previous baseline exactly (pinned below).
    ///
    /// 2026-10-30, the control blades (MELCOR's NS component, steel only;
    /// `control_blades`): 97 blades of 60 kg \[A\] at their cells' cladding
    /// temperature, MELCOR's stainless-steel oxidation, failing at 1700 K and
    /// draining to the lower plenum, where the steel quenches. Lower head
    /// 4.5914 -> 5.4506 h, depressurised 4.5914 -> 5.1678 h (the steam line
    /// creeps first), H2 247.99 -> 474.93 kg (now inside the nine codes'
    /// 398-1315 kg), drywell peak 0.96498045 -> 0.81957198 MPa, UO2 5.052 ->
    /// 3.140 kg. Predicted (written first): heat-up slower (held: 1172 K
    /// against 1208 K at 3.25 h), H2 within 25 kg and up 5-40 kg by 4 h
    /// (both missed), the head inside 4.2-6.2 h (held). The steel's own
    /// oxidation is small (262 kg of steel, about 10 kg of H2); the H2 comes
    /// from 5.8 t of blade steel draining at 1700 K into the plenum's water
    /// from about 4.3 h: its quench raises about 2.5 t of steam, which ends
    /// the core's steam starvation, and the Zr oxidised rises from 6.4 to
    /// 10.6 t. The molten steel's heat of fusion is left out (it leaves at
    /// its solid enthalpy at 1700 K, stated). `without_control_blades`
    /// reproduces the previous baseline exactly (pinned below).
    ///
    /// 2026-10-30, the blade's heat of fusion (MELCOR's 2.68e5 J/kg,
    /// `blade_fusion`): a blade at 1700 K holds its cell there while it
    /// melts, and drains carrying that heat. Lower head 5.4506 -> 5.2403 h
    /// (the steam line no longer fails first: depressurised 5.1678 ->
    /// 5.2403 h), H2 474.93 -> 449.03 kg, drywell peak 0.81957198 ->
    /// 0.81434262 MPa, UO2 3.140 -> 2.891 kg. Predicted (written first): H2
    /// up 20-80 kg (the sign missed), the head within 0.3 h and the peak
    /// within 0.05 MPa (both held). Likely reason for the sign: the hold at
    /// 1700 K slows the hottest nodes where the oxidation rate climbs
    /// steeply, and delays the drain, which outweighs the drain's larger
    /// heat (not separately measured). `without_blade_fusion` reproduces the
    /// previous baseline exactly (pinned below).
    ///
    /// 2026-10-30, the cavity debris's phases (MELCOR CAV RM §3.3, §3.10;
    /// `cavity_phases`): an ideal-solution pseudo-binary melting range with
    /// latent heats, the interior conducting (`6 k / H`) when solid. Drywell
    /// peak 0.81434262 -> 0.74999700 MPa (held at the flange leak's opening
    /// pressure); the in-vessel metrics unchanged. The debris reaches the
    /// floor at about 1300 K, solid, keeps its decay heat, passes 2900 K by
    /// 9 h and ablates 71 t of concrete by 25 h; its Zr gives 265 kg of H2
    /// and 385 kg of CO in the drywell, which holds 100-140 kg of H2 from
    /// 6 h; the drywell rides a 0.70-0.75 MPa plateau from 16 h, the flange
    /// open from 16.7 h, and the building reaches 6.8 % H2 by 24 h.
    /// Predicted (written first): cavity H2 50-400 kg, drywell H2 50-300 kg,
    /// the flange earlier, in-vessel unchanged (all held); drywell up
    /// 0.05-0.3 MPa over 10-24 h (partly: +0.01 at 12 h, -0.03 at 20 h once
    /// the flange bleeds); building H2 0.5-6 % (6.8 %, just above).
    /// `without_cavity_phases` reproduces the previous baseline exactly
    /// (pinned below).
    ///
    /// 2026-10-30, the building's ignition by MELCOR's BUR criteria
    /// (`bur_ignition`): no change (the building's Le Chatelier Y peaks at
    /// 0.0714, below 0.10). Then the vent and the flange limited to the
    /// critical gas flow (MELCOR CVH/FL RM Eq 6-13, `gas_choking`): drywell
    /// peak 0.74999700 -> 0.80352739 MPa, in-vessel unchanged. Predicted
    /// (written first): the peak 0.77-0.83 MPa (held), the building's H2 at
    /// 26 h 3.5-5 % (3.28 %, narrowly missed), the drywell at 24 h 0.25-0.4
    /// MPa (0.198: missed), in-vessel unchanged (held).
    /// `without_gas_choking` reproduces the previous baseline exactly
    /// (pinned below).
    ///
    /// 2026-10-30, the blades' steel oxidation held to MELCOR's gaseous-
    /// diffusion limit (COR RM Eq 2-168, `blade_diffusion_limit`): head
    /// 5.2403 -> 5.2381 h, H2 449.03 -> 448.16 kg, drywell peak 0.80352739 ->
    /// 0.80291028 MPa. Predicted (written first): H2 within 10 kg, the head
    /// within 0.05 h, the peak within 0.01 MPa; all held.
    /// `without_blade_diffusion_limit` reproduces the previous baseline
    /// exactly (pinned below).
    ///
    /// 2026-10-30, a numerical defect fixed (`two_sided_equalising`): the
    /// downcomers' and vacuum breakers' equalising cap counted only the
    /// donor's pressure fall, overshot the set point, and the vacuum breaker
    /// chattered about it every step from 2.55 h, amplifying a 1e-9
    /// perturbation 1.5 times a step. Head 5.2381 -> 5.4036 h, depressurised
    /// 5.2381 -> 5.1031 h (the steam line first), H2 448.16 -> 479.93 kg,
    /// drywell peak 0.80291028 -> 0.80577760 MPa: all inside the ensemble's
    /// spread (`verification_and_validation/bwr_mark1/ensemble.md`), so this
    /// single run's move is not evidence of physics. Predicted (written
    /// first): the chatter gone (held: the difference sits at 3400.9 Pa,
    /// steady); the 1e-9 sensitivity down 100x (missed: pool water still
    /// gives 5.4e7, the first divergence now at 3.49 h in the core, a
    /// threshold event that is not a relocation); single-run metrics inside
    /// the spread (held). `without_two_sided_equalising` reproduces the
    /// previous baseline exactly (pinned below).
    ///
    /// When a plan item moves these numbers, this test changes in the same
    /// commit with the old value, the new one and why (never a quiet
    /// repair).
    /// The ensemble (2026-10-30): a member with `rel = 0` is the default
    /// run exactly; the report's nearest-rank quartiles by hand on four
    /// members (head 1, 2, 3, 4 h: quartiles 1, 2, 3; max 4).
    #[test]
    fn the_ensemble_reproduces_the_default_and_reports_quartiles() {
        assert_eq!(ensemble_member(7, 0.0, 1), measure(Mechanisms::default()));
        let m = |h: f64| Metrics { lower_head_h: Some(h), depressurised_h: Some(h), h2_25h_kg: 10.0 * h, p_dw_peak_mpa: 0.1 * h, vent_h: None, explosion_h: None, fp_released_25h: 1.0, uo2_released_25h_kg: 1.0 };
        let r = ensemble_report(&[m(3.0), m(1.0), m(4.0), m(2.0)]);
        assert!(r.contains("| lower-head failure, h | 1.0000 | 1.0000 | 2.0000 | 3.0000 | 4.0000 |"), "{r}");
        assert!(r.contains("the vent opened in 0, the building exploded in 0"), "{r}");
    }

    #[test]
    fn the_uncalibrated_baseline_is_pinned() {
        let m = measure(Mechanisms::default());
        // The earlier models below are reproduced with the fission
        // products' decay heat left in the core, as it was then.
        let base = Mechanisms::default().without_fp_decay_follows().without_te_transport().without_debris_oxidation().without_candling().without_debris_quench().without_wet_head_contact().without_failed_vessel_drain().without_ww_atmosphere().without_dw_wall_hs().without_ww_shell().without_outflow_enthalpy().without_relief_h2_enthalpy().without_condensate_to_floor().without_rpv_wall().without_internals_radiation().without_core_top_radiation().without_see_through().without_mcci().without_dw_wall_outer().without_core_plate().without_bottom_radiation().without_ring_plates().without_pool_layers().without_plate_oxidation().without_lp_steam_consumed().without_control_blades().without_blade_fusion().without_cavity_phases().without_bur_ignition().without_gas_choking().without_blade_diffusion_limit().without_two_sided_equalising();
        let near = |a: Option<f64>, b: f64, tol: f64| a.is_some_and(|x| (x - b).abs() <= tol);
        assert!(near(m.lower_head_h, 5.4036, 0.001), "{m:?}");
        assert!(near(m.depressurised_h, 5.1031, 0.001), "{m:?}");
        assert!((m.h2_25h_kg - 479.93).abs() < 0.01, "{m:?}");
        assert!((m.p_dw_peak_mpa - 0.80577760).abs() < 1e-8 && (m.uo2_released_25h_kg - 2.877).abs() < 0.01, "{m:?}");
        // The one-sided equalising cap: the previous baseline, exactly.
        let eq = measure(Mechanisms::default().without_two_sided_equalising());
        assert!(near(eq.lower_head_h, 5.2381, 0.001) && (eq.h2_25h_kg - 448.16).abs() < 0.01, "{eq:?}");
        assert!((eq.p_dw_peak_mpa - 0.80291028).abs() < 1e-8 && (eq.uo2_released_25h_kg - 2.890).abs() < 0.01, "{eq:?}");
        // The blades' steel without the diffusion limit: the previous
        // baseline, exactly.
        let bd = measure(Mechanisms::default().without_blade_diffusion_limit().without_two_sided_equalising());
        assert!(near(bd.lower_head_h, 5.2403, 0.001) && (bd.h2_25h_kg - 449.03).abs() < 0.01, "{bd:?}");
        assert!((bd.p_dw_peak_mpa - 0.80352739).abs() < 1e-8 && (bd.uo2_released_25h_kg - 2.891).abs() < 0.01, "{bd:?}");
        // The vent and the flange as incompressible orifices: the previous
        // baseline, exactly (the BUR ignition changes nothing in this run).
        let gc = measure(Mechanisms::default().without_gas_choking().without_blade_diffusion_limit().without_two_sided_equalising());
        assert!((gc.p_dw_peak_mpa - 0.74999700).abs() < 1e-8 && (gc.h2_25h_kg - 449.03).abs() < 0.01, "{gc:?}");
        // The cavity debris convecting without phase change: the previous
        // baseline, exactly.
        let cp2 = measure(Mechanisms::default().without_cavity_phases().without_bur_ignition().without_gas_choking().without_blade_diffusion_limit().without_two_sided_equalising());
        assert!(near(cp2.lower_head_h, 5.2403, 0.001) && (cp2.h2_25h_kg - 449.03).abs() < 0.01, "{cp2:?}");
        assert!((cp2.p_dw_peak_mpa - 0.81434262).abs() < 1e-8, "{cp2:?}");
        // The blade drained at once at its solid enthalpy: the previous
        // baseline, exactly.
        let bf = measure(Mechanisms::default().without_blade_fusion().without_cavity_phases().without_bur_ignition().without_gas_choking().without_blade_diffusion_limit().without_two_sided_equalising());
        assert!(near(bf.lower_head_h, 5.4506, 0.001) && near(bf.depressurised_h, 5.1678, 0.001), "{bf:?}");
        assert!((bf.h2_25h_kg - 474.93).abs() < 0.01 && (bf.p_dw_peak_mpa - 0.81957198).abs() < 1e-8, "{bf:?}");
        // A core without control blades: the previous baseline, exactly.
        let cb = measure(Mechanisms::default().without_control_blades().without_blade_fusion().without_cavity_phases().without_bur_ignition().without_gas_choking().without_blade_diffusion_limit().without_two_sided_equalising());
        assert!(near(cb.lower_head_h, 4.5914, 0.001) && (cb.h2_25h_kg - 247.99).abs() < 0.01, "{cb:?}");
        assert!((cb.p_dw_peak_mpa - 0.96498045).abs() < 1e-8 && (cb.uo2_released_25h_kg - 5.052).abs() < 0.01, "{cb:?}");
        // The lower plenum's oxidation steam rising into the core as well
        // (the defect): the previous baseline, exactly.
        let ls = measure(Mechanisms::default().without_lp_steam_consumed().without_control_blades().without_blade_fusion().without_cavity_phases().without_bur_ignition().without_gas_choking().without_blade_diffusion_limit().without_two_sided_equalising());
        assert!(near(ls.lower_head_h, 4.5914, 0.001) && (ls.h2_25h_kg - 252.87).abs() < 0.01, "{ls:?}");
        assert!((ls.p_dw_peak_mpa - 0.96523341).abs() < 1e-8 && (ls.uo2_released_25h_kg - 4.630).abs() < 0.01, "{ls:?}");
        // The debris on the plate inert: the previous baseline, exactly.
        let po = measure(Mechanisms::default().without_plate_oxidation().without_lp_steam_consumed().without_control_blades().without_blade_fusion().without_cavity_phases().without_bur_ignition().without_gas_choking().without_blade_diffusion_limit().without_two_sided_equalising());
        assert!(near(po.lower_head_h, 4.5914, 0.001) && (po.h2_25h_kg - 256.41).abs() < 0.01, "{po:?}");
        assert!((po.p_dw_peak_mpa - 0.96464924).abs() < 1e-8 && (po.uo2_released_25h_kg - 4.525).abs() < 0.01, "{po:?}");
        // One well-mixed pool: the previous baseline, exactly.
        let pl = measure(Mechanisms::default().without_pool_layers().without_plate_oxidation().without_lp_steam_consumed().without_control_blades().without_blade_fusion().without_cavity_phases().without_bur_ignition().without_gas_choking().without_blade_diffusion_limit().without_two_sided_equalising());
        assert!(near(pl.lower_head_h, 4.5900, 0.001) && (pl.h2_25h_kg - 253.50).abs() < 0.01, "{pl:?}");
        assert!((pl.p_dw_peak_mpa - 0.75497232).abs() < 1e-8 && (pl.uo2_released_25h_kg - 4.647).abs() < 0.01, "{pl:?}");
        // One plate under the whole core: the previous baseline, exactly.
        let rp = measure(Mechanisms::default().without_ring_plates().without_pool_layers().without_plate_oxidation().without_lp_steam_consumed().without_control_blades().without_blade_fusion().without_cavity_phases().without_bur_ignition().without_gas_choking().without_blade_diffusion_limit().without_two_sided_equalising());
        assert!(near(rp.lower_head_h, 5.9828, 0.001) && near(rp.depressurised_h, 5.2419, 0.001), "{rp:?}");
        assert!((rp.h2_25h_kg - 296.36).abs() < 0.01 && (rp.p_dw_peak_mpa - 0.56079567).abs() < 1e-8, "{rp:?}");
        assert!((rp.uo2_released_25h_kg - 2.847).abs() < 0.01 && near(rp.vent_h, 24.1631, 0.001), "{rp:?}");
        // The bottom of the core exchanging nothing: the previous
        // baseline, exactly.
        let br = measure(Mechanisms::default().without_bottom_radiation().without_ring_plates().without_pool_layers().without_plate_oxidation().without_lp_steam_consumed().without_control_blades().without_blade_fusion().without_cavity_phases().without_bur_ignition().without_gas_choking().without_blade_diffusion_limit().without_two_sided_equalising());
        assert!(near(br.lower_head_h, 5.0003, 0.001) && near(br.depressurised_h, 5.0003, 0.001), "{br:?}");
        assert!((br.h2_25h_kg - 183.59).abs() < 0.01 && (br.p_dw_peak_mpa - 0.64073627).abs() < 1e-8, "{br:?}");
        // No core-plate holdup: the previous baseline, exactly.
        let cp = measure(Mechanisms::default().without_core_plate().without_bottom_radiation().without_ring_plates().without_pool_layers().without_plate_oxidation().without_lp_steam_consumed().without_control_blades().without_blade_fusion().without_cavity_phases().without_bur_ignition().without_gas_choking().without_blade_diffusion_limit().without_two_sided_equalising());
        assert!(near(cp.lower_head_h, 4.6225, 0.001) && (cp.h2_25h_kg - 226.71).abs() < 0.01, "{cp:?}");
        assert!((cp.p_dw_peak_mpa - 0.68842913).abs() < 1e-8 && (cp.uo2_released_25h_kg - 6.598).abs() < 0.01, "{cp:?}");
        // The structures adiabatic outside: the previous baseline, exactly.
        let wo = measure(Mechanisms::default().without_dw_wall_outer().without_core_plate().without_bottom_radiation().without_ring_plates().without_pool_layers().without_plate_oxidation().without_lp_steam_consumed().without_control_blades().without_blade_fusion().without_cavity_phases().without_bur_ignition().without_gas_choking().without_blade_diffusion_limit().without_two_sided_equalising());
        assert!((wo.p_dw_peak_mpa - 0.77350757).abs() < 1e-8 && (wo.h2_25h_kg - 226.85).abs() < 0.01, "{wo:?}");
        // No core-concrete interaction: the previous baseline, exactly.
        let mc = measure(Mechanisms::default().without_mcci().without_dw_wall_outer().without_core_plate().without_bottom_radiation().without_ring_plates().without_pool_layers().without_plate_oxidation().without_lp_steam_consumed().without_control_blades().without_blade_fusion().without_cavity_phases().without_bur_ignition().without_gas_choking().without_blade_diffusion_limit().without_two_sided_equalising());
        assert!((mc.p_dw_peak_mpa - 0.56765688).abs() < 1e-8 && (mc.uo2_released_25h_kg - 6.598).abs() < 0.01, "{mc:?}");
        // No view across emptied cells: the previous baseline, exactly.
        let st = measure(Mechanisms::default().without_see_through().without_mcci().without_dw_wall_outer().without_core_plate().without_bottom_radiation().without_ring_plates().without_pool_layers().without_plate_oxidation().without_lp_steam_consumed().without_control_blades().without_blade_fusion().without_cavity_phases().without_bur_ignition().without_gas_choking().without_blade_diffusion_limit().without_two_sided_equalising());
        assert!((st.h2_25h_kg - 226.81).abs() < 0.01 && (st.p_dw_peak_mpa - 0.56772094).abs() < 1e-8, "{st:?}");
        // The top of the core exchanging nothing: the previous baseline,
        // exactly.
        let tr = measure(Mechanisms::default().without_core_top_radiation().without_see_through().without_mcci().without_dw_wall_outer().without_core_plate().without_bottom_radiation().without_ring_plates().without_pool_layers().without_plate_oxidation().without_lp_steam_consumed().without_control_blades().without_blade_fusion().without_cavity_phases().without_bur_ignition().without_gas_choking().without_blade_diffusion_limit().without_two_sided_equalising());
        assert!((tr.h2_25h_kg - 227.86).abs() < 0.01 && (tr.p_dw_peak_mpa - 0.57458213).abs() < 1e-8, "{tr:?}");
        // The internals not radiating: the previous baseline, exactly.
        let ir = measure(Mechanisms::default().without_internals_radiation().without_core_top_radiation().without_see_through().without_mcci().without_dw_wall_outer().without_core_plate().without_bottom_radiation().without_ring_plates().without_pool_layers().without_plate_oxidation().without_lp_steam_consumed().without_control_blades().without_blade_fusion().without_cavity_phases().without_bur_ignition().without_gas_choking().without_blade_diffusion_limit().without_two_sided_equalising());
        assert!(near(ir.lower_head_h, 4.6236, 0.001) && (ir.h2_25h_kg - 228.61).abs() < 0.01, "{ir:?}");
        assert!((ir.p_dw_peak_mpa - 0.57523829).abs() < 1e-8, "{ir:?}");
        // No RPV wall: the previous baseline, exactly.
        let rw = measure(Mechanisms::default().without_rpv_wall().without_internals_radiation().without_core_top_radiation().without_see_through().without_mcci().without_dw_wall_outer().without_core_plate().without_bottom_radiation().without_ring_plates().without_pool_layers().without_plate_oxidation().without_lp_steam_consumed().without_control_blades().without_blade_fusion().without_cavity_phases().without_bur_ignition().without_gas_choking().without_blade_diffusion_limit().without_two_sided_equalising());
        assert!(near(rw.lower_head_h, 4.5558, 0.001) && (rw.h2_25h_kg - 253.13).abs() < 0.01, "{rw:?}");
        assert!((rw.p_dw_peak_mpa - 0.58660172).abs() < 1e-8 && (rw.uo2_released_25h_kg - 4.891).abs() < 0.01, "{rw:?}");
        // The drywell's condensate leaving the model: the previous
        // baseline, exactly.
        let cf = measure(Mechanisms::default().without_condensate_to_floor().without_rpv_wall().without_internals_radiation().without_core_top_radiation().without_see_through().without_mcci().without_dw_wall_outer().without_core_plate().without_bottom_radiation().without_ring_plates().without_pool_layers().without_plate_oxidation().without_lp_steam_consumed().without_control_blades().without_blade_fusion().without_cavity_phases().without_bur_ignition().without_gas_choking().without_blade_diffusion_limit().without_two_sided_equalising());
        assert!((cf.p_dw_peak_mpa - 0.56658590).abs() < 1e-8, "{cf:?}");
        // The relief valves' hydrogen at the pool's temperature: the
        // previous baseline, exactly.
        let rh = measure(Mechanisms::default().without_relief_h2_enthalpy().without_condensate_to_floor().without_rpv_wall().without_internals_radiation().without_core_top_radiation().without_see_through().without_mcci().without_dw_wall_outer().without_core_plate().without_bottom_radiation().without_ring_plates().without_pool_layers().without_plate_oxidation().without_lp_steam_consumed().without_control_blades().without_blade_fusion().without_cavity_phases().without_bur_ignition().without_gas_choking().without_blade_diffusion_limit().without_two_sided_equalising());
        assert!((rh.p_dw_peak_mpa - 0.56640504).abs() < 1e-8, "{rh:?}");
        // Drywell outflows at internal energy: the previous baseline, exactly.
        let of = measure(Mechanisms::default().without_outflow_enthalpy().without_relief_h2_enthalpy().without_condensate_to_floor().without_rpv_wall().without_internals_radiation().without_core_top_radiation().without_see_through().without_mcci().without_dw_wall_outer().without_core_plate().without_bottom_radiation().without_ring_plates().without_pool_layers().without_plate_oxidation().without_lp_steam_consumed().without_control_blades().without_blade_fusion().without_cavity_phases().without_bur_ignition().without_gas_choking().without_blade_diffusion_limit().without_two_sided_equalising());
        assert!((of.p_dw_peak_mpa - 0.56079765).abs() < 1e-8 && near(of.vent_h, 24.0133, 0.001), "{of:?}");
        // No torus shell: the previous baseline, exactly.
        let sh = measure(Mechanisms::default().without_ww_shell().without_outflow_enthalpy().without_relief_h2_enthalpy().without_condensate_to_floor().without_rpv_wall().without_internals_radiation().without_core_top_radiation().without_see_through().without_mcci().without_dw_wall_outer().without_core_plate().without_bottom_radiation().without_ring_plates().without_pool_layers().without_plate_oxidation().without_lp_steam_consumed().without_control_blades().without_blade_fusion().without_cavity_phases().without_bur_ignition().without_gas_choking().without_blade_diffusion_limit().without_two_sided_equalising());
        assert!((sh.p_dw_peak_mpa - 0.56079836).abs() < 1e-8 && near(sh.vent_h, 23.8872, 0.001), "{sh:?}");
        // The drywell structures with the fixed coefficient: the previous
        // baseline, exactly.
        let hs = measure(Mechanisms::default().without_dw_wall_hs().without_ww_shell().without_outflow_enthalpy().without_relief_h2_enthalpy().without_condensate_to_floor().without_rpv_wall().without_internals_radiation().without_core_top_radiation().without_see_through().without_mcci().without_dw_wall_outer().without_core_plate().without_bottom_radiation().without_ring_plates().without_pool_layers().without_plate_oxidation().without_lp_steam_consumed().without_control_blades().without_blade_fusion().without_cavity_phases().without_bur_ignition().without_gas_choking().without_blade_diffusion_limit().without_two_sided_equalising());
        assert!((hs.p_dw_peak_mpa - 0.56194352).abs() < 1e-8 && near(hs.vent_h, 23.7336, 0.001), "{hs:?}");
        // The wetwell gas at the pool's temperature, saturated: the previous
        // baseline, exactly.
        let wa = measure(Mechanisms::default().without_ww_atmosphere().without_dw_wall_hs().without_ww_shell().without_outflow_enthalpy().without_relief_h2_enthalpy().without_condensate_to_floor().without_rpv_wall().without_internals_radiation().without_core_top_radiation().without_see_through().without_mcci().without_dw_wall_outer().without_core_plate().without_bottom_radiation().without_ring_plates().without_pool_layers().without_plate_oxidation().without_lp_steam_consumed().without_control_blades().without_blade_fusion().without_cavity_phases().without_bur_ignition().without_gas_choking().without_blade_diffusion_limit().without_two_sided_equalising());
        assert!(near(wa.lower_head_h, 4.5558, 0.001) && (wa.h2_25h_kg - 253.13).abs() < 0.01, "{wa:?}");
        assert!((wa.p_dw_peak_mpa - 0.58862947).abs() < 1e-8 && near(wa.vent_h, 23.7336, 0.001), "{wa:?}");
        // The failed vessel keeping its water: the previous baseline, exactly.
        let dr = measure(Mechanisms::default().without_failed_vessel_drain().without_ww_atmosphere().without_dw_wall_hs().without_ww_shell().without_outflow_enthalpy().without_relief_h2_enthalpy().without_condensate_to_floor().without_rpv_wall().without_internals_radiation().without_core_top_radiation().without_see_through().without_mcci().without_dw_wall_outer().without_core_plate().without_bottom_radiation().without_ring_plates().without_pool_layers().without_plate_oxidation().without_lp_steam_consumed().without_control_blades().without_blade_fusion().without_cavity_phases().without_bur_ignition().without_gas_choking().without_blade_diffusion_limit().without_two_sided_equalising());
        assert!(near(dr.lower_head_h, 4.5558, 0.001) && (dr.h2_25h_kg - 253.13).abs() < 0.01, "{dr:?}");
        assert!((dr.p_dw_peak_mpa - 0.58922450).abs() < 1e-8 && (dr.fp_released_25h - 0.9955).abs() < 1e-4, "{dr:?}");
        // Vent 23.7336 -> 24.1631 h and UO2 6.487 -> 2.847 kg with the
        // bottom of the core's radiation; back to 23.7336 h, UO2 4.647 kg,
        // with a plate per ring (2026-10-29).
        assert!(near(m.vent_h, 23.7336, 0.001) && m.explosion_h.is_none(), "{m:?}");
        assert!((m.fp_released_25h - 0.9977).abs() < 1e-4 && (m.uo2_released_25h_kg - 2.877).abs() < 0.01, "{m:?}");
        // No wet head contact: the previous baseline, exactly (penetration
        // coverage then binds nowhere: the bed is deeper than the bottom
        // level by the time the plenum dries).
        let wh = measure(Mechanisms::default().without_wet_head_contact().without_failed_vessel_drain().without_ww_atmosphere().without_dw_wall_hs().without_ww_shell().without_outflow_enthalpy().without_relief_h2_enthalpy().without_condensate_to_floor().without_rpv_wall().without_internals_radiation().without_core_top_radiation().without_see_through().without_mcci().without_dw_wall_outer().without_core_plate().without_bottom_radiation().without_ring_plates().without_pool_layers().without_plate_oxidation().without_lp_steam_consumed().without_control_blades().without_blade_fusion().without_cavity_phases().without_bur_ignition().without_gas_choking().without_blade_diffusion_limit().without_two_sided_equalising());
        assert!(near(wh.lower_head_h, 4.7364, 0.001) && near(wh.depressurised_h, 4.6206, 0.001), "{wh:?}");
        assert!((wh.h2_25h_kg - 395.54).abs() < 0.01 && (wh.p_dw_peak_mpa - 0.72921107).abs() < 1e-8, "{wh:?}");
        assert!((wh.fp_released_25h - 0.9978).abs() < 1e-4 && (wh.uo2_released_25h_kg - 3.949).abs() < 0.01, "{wh:?}");
        // Wet contact without the coverage: the head fails at the first
        // debris (diagnostic on the current model, pinned: 3.5539 h and
        // 88.73 kg until the RPV wall, 3.6353 h and 77.48 kg since; 3.6356 h
        // and 77.33 kg since the internals radiate; 3.6358 h and 77.38 kg on
        // the one steel emissivity; 3.6356 h and 77.20 kg since the top of
        // the core radiates; with MCCI it vents at the order, 23.73 h, where
        // it did not vent before; 3.6353 h and 77.10 kg with the drywell's
        // outer heat path). Since the core plate holds the first debris it
        // no longer fails at the first debris: 4.9972 h and 173.43 kg, 0.003
        // h before the coverage case (the plate's failure, not the head's
        // contact, now sets the time). With the bottom of the core's
        // radiation, 4.9989 h and 190.53 kg: this case stays on the branch
        // where the head fails as the plate's debris lands, which the
        // baseline (head 5.98 h) has left; a second sign that the head's
        // failure near 5.0 h is a knife edge. With a plate per ring, 4.5733 h
        // and 200.29 kg; with the pool in two layers 4.5747 h, 200.62 kg;
        // with the plenum's oxidation steam consumed, 198.83 kg; with the
        // control blades 4.4794 h and 217.93 kg (this case keeps the early
        // branch the baseline has left); with their heat of fusion 4.4822 h
        // and 231.31 kg; with the vent and flange choked, 231.26 kg; with the
        // blades' diffusion limit 4.4828 h, 232.43 kg; with the two-sided
        // equalising 5.4036 h and 479.93 kg, the baseline's own (the
        // coverage no longer binds in this trajectory).
        let nc = measure(Mechanisms::default().without_penetration_coverage());
        assert!(near(nc.lower_head_h, 5.4036, 0.001) && (nc.h2_25h_kg - 479.93).abs() < 0.01 && near(nc.vent_h, 23.7336, 0.001), "{nc:?}");
        // MELCOR's quench off: the baseline before it, exactly.
        let dq = measure(Mechanisms::default().without_debris_quench().without_wet_head_contact().without_failed_vessel_drain().without_ww_atmosphere().without_dw_wall_hs().without_ww_shell().without_outflow_enthalpy().without_relief_h2_enthalpy().without_condensate_to_floor().without_rpv_wall().without_internals_radiation().without_core_top_radiation().without_see_through().without_mcci().without_dw_wall_outer().without_core_plate().without_bottom_radiation().without_ring_plates().without_pool_layers().without_plate_oxidation().without_lp_steam_consumed().without_control_blades().without_blade_fusion().without_cavity_phases().without_bur_ignition().without_gas_choking().without_blade_diffusion_limit().without_two_sided_equalising());
        assert!(near(dq.lower_head_h, 5.3144, 0.001) && near(dq.depressurised_h, 4.5206, 0.001), "{dq:?}");
        assert!((dq.h2_25h_kg - 441.97).abs() < 0.01 && (dq.p_dw_peak_mpa - 0.70450517).abs() < 1e-8, "{dq:?}");
        assert!((dq.fp_released_25h - 0.9911).abs() < 1e-4 && (dq.uo2_released_25h_kg - 2.780).abs() < 0.01, "{dq:?}");
        // No candling: the baseline before it, exactly.
        let cd = measure(Mechanisms::default().without_candling().without_debris_quench().without_wet_head_contact().without_failed_vessel_drain().without_ww_atmosphere().without_dw_wall_hs().without_ww_shell().without_outflow_enthalpy().without_relief_h2_enthalpy().without_condensate_to_floor().without_rpv_wall().without_internals_radiation().without_core_top_radiation().without_see_through().without_mcci().without_dw_wall_outer().without_core_plate().without_bottom_radiation().without_ring_plates().without_pool_layers().without_plate_oxidation().without_lp_steam_consumed().without_control_blades().without_blade_fusion().without_cavity_phases().without_bur_ignition().without_gas_choking().without_blade_diffusion_limit().without_two_sided_equalising());
        assert!(near(cd.lower_head_h, 4.4831, 0.001) && near(cd.depressurised_h, 4.4489, 0.001), "{cd:?}");
        assert!((cd.h2_25h_kg - 313.43).abs() < 0.01 && (cd.p_dw_peak_mpa - 0.62303386).abs() < 1e-8, "{cd:?}");
        assert!((cd.fp_released_25h - 0.8109).abs() < 1e-4 && (cd.uo2_released_25h_kg - 3.516).abs() < 0.01, "{cd:?}");
        // No debris oxidation: the baseline before it, exactly.
        let dox = measure(Mechanisms::default().without_debris_oxidation().without_candling().without_debris_quench().without_wet_head_contact().without_failed_vessel_drain().without_ww_atmosphere().without_dw_wall_hs().without_ww_shell().without_outflow_enthalpy().without_relief_h2_enthalpy().without_condensate_to_floor().without_rpv_wall().without_internals_radiation().without_core_top_radiation().without_see_through().without_mcci().without_dw_wall_outer().without_core_plate().without_bottom_radiation().without_ring_plates().without_pool_layers().without_plate_oxidation().without_lp_steam_consumed().without_control_blades().without_blade_fusion().without_cavity_phases().without_bur_ignition().without_gas_choking().without_blade_diffusion_limit().without_two_sided_equalising());
        assert!(near(dox.lower_head_h, 4.5172, 0.001) && near(dox.depressurised_h, 4.4497, 0.001), "{dox:?}");
        assert!((dox.h2_25h_kg - 305.64).abs() < 0.01 && (dox.p_dw_peak_mpa - 0.60869364).abs() < 1e-8, "{dox:?}");
        assert!((dox.fp_released_25h - 0.7977).abs() < 1e-4 && (dox.uo2_released_25h_kg - 3.645).abs() < 0.01, "{dox:?}");
        // No tellurium transport: the baseline before it, exactly.
        let te = measure(Mechanisms::default().without_te_transport().without_debris_oxidation().without_candling().without_debris_quench().without_wet_head_contact().without_failed_vessel_drain().without_ww_atmosphere().without_dw_wall_hs().without_ww_shell().without_outflow_enthalpy().without_relief_h2_enthalpy().without_condensate_to_floor().without_rpv_wall().without_internals_radiation().without_core_top_radiation().without_see_through().without_mcci().without_dw_wall_outer().without_core_plate().without_bottom_radiation().without_ring_plates().without_pool_layers().without_plate_oxidation().without_lp_steam_consumed().without_control_blades().without_blade_fusion().without_cavity_phases().without_bur_ignition().without_gas_choking().without_blade_diffusion_limit().without_two_sided_equalising());
        assert!(near(te.lower_head_h, 4.5239, 0.001) && near(te.depressurised_h, 4.4361, 0.001), "{te:?}");
        assert!((te.h2_25h_kg - 304.26).abs() < 0.01 && (te.p_dw_peak_mpa - 0.61279710).abs() < 1e-8, "{te:?}");
        assert!((te.fp_released_25h - 0.7967).abs() < 1e-4 && (te.uo2_released_25h_kg - 3.327).abs() < 0.01, "{te:?}");
        // Fission products' decay heat left in the core: the previous
        // baseline, exactly.
        let fd = measure(base);
        assert!(near(fd.lower_head_h, 4.4692, 0.001) && near(fd.depressurised_h, 4.4061, 0.001), "{fd:?}");
        assert!((fd.h2_25h_kg - 295.45).abs() < 0.01 && (fd.p_dw_peak_mpa - 0.59492996).abs() < 1e-8, "{fd:?}");
        assert!((fd.fp_released_25h - 0.7989).abs() < 1e-4 && (fd.uo2_released_25h_kg - 2.634).abs() < 0.01, "{fd:?}");
        // No internals heat loss: the previous baseline, exactly.
        let il = measure(base.without_internals_heat_loss());
        assert!(near(il.lower_head_h, 5.4517, 0.001) && near(il.depressurised_h, 4.7769, 0.001), "{il:?}");
        assert!((il.h2_25h_kg - 215.36).abs() < 0.01, "{il:?}");
        assert!((il.p_dw_peak_mpa - 0.55680172).abs() < 1e-8, "{il:?}");
        // No failed-vessel outflow: the previous baseline, exactly.
        let fo = measure(base.without_failed_vessel_outflow().without_internals_heat_loss());
        assert!((fo.p_dw_peak_mpa - 0.55678708).abs() < 1e-8 && fo.lower_head_h == il.lower_head_h, "{fo:?}");
        // Volatile fission products released from the fuel, in-vessel
        // (CORSOR-M, gh:#1040; output only, the fixture did not move):
        // 0.8505 by 25 h, released in steps with the core's degradation
        // (0.26 at 4.5 h, 0.67 at 6 h, 0.85 by 8 h). Predicted before the
        // run, once the ex-vessel artefact was removed: 0.4-0.7. **Missed,
        // high**: the nodes left in the core heat and release before they
        // fall. BSAF reports only that most codes exceeded 0.8 (Fig. 2.5,
        // code-to-code), which this is consistent with.
        // (Since the internals heat loss these are the previous baseline's.)
        assert!((il.fp_released_25h - 0.8505).abs() < 1e-4, "{il:?}");
        // UO2 released from the fuel (CORSOR-M's UO2 row; the seed aerosol,
        // gh:#1047; output only): 3.35 kg by 25 h (0.03 kg by 5 h, 2.5 kg
        // by 8 h). Predicted before the run: tens of kg. **Missed, low**, by
        // about 5-10x: few nodes hold near 2500 K for long.
        assert!((il.uo2_released_25h_kg - 3.346).abs() < 0.01, "{il:?}");
        // No penetrations: the previous baseline, exactly.
        let np = measure(base.without_penetrations().without_failed_vessel_outflow().without_internals_heat_loss());
        assert!(near(np.lower_head_h, 5.6392, 0.001) && near(np.depressurised_h, 4.7769, 0.001), "{np:?}");
        assert!((np.h2_25h_kg - 215.36).abs() < 0.01, "{np:?}");
        assert!((np.p_dw_peak_mpa - 0.55625090).abs() < 1e-8, "{np:?}");
        assert!(il.vent_h.is_none() && il.explosion_h.is_none(), "{il:?}");
        // No dome internals: the previous baseline, exactly.
        let di = measure(base.without_dome_internals().without_penetrations().without_failed_vessel_outflow().without_internals_heat_loss());
        assert!(near(di.lower_head_h, 5.4492, 0.001) && near(di.depressurised_h, 4.3583, 0.001), "{di:?}");
        assert!((di.h2_25h_kg - 191.43).abs() < 0.01, "{di:?}");
        assert!((di.p_dw_peak_mpa - 0.54684943).abs() < 1e-8, "{di:?}");
        // No steam-line creep: the previous baseline, exactly (with the
        // dry-vessel enthalpy fix: 5.3925 -> 5.3806 h, 193.97 -> 192.59 kg,
        // 0.53474702 -> 0.53460139 MPa).
        let ml = measure(base.without_msl_creep().without_dome_internals().without_penetrations().without_failed_vessel_outflow().without_internals_heat_loss());
        assert!(near(ml.lower_head_h, 5.3806, 0.001) && near(ml.depressurised_h, 5.3806, 0.001), "{ml:?}");
        assert!((ml.h2_25h_kg - 192.59).abs() < 0.01, "{ml:?}");
        assert!((ml.p_dw_peak_mpa - 0.53460139).abs() < 1e-8, "{ml:?}");
        // Constant hydrogen c_p: the previous baseline, exactly (with the
        // dry-vessel enthalpy fix: 5.390 -> 5.3808 h, 192.11 -> 190.95 kg,
        // 0.53414290 -> 0.53396739 MPa).
        let hc = measure(base.without_hydrogen_cp_t().without_msl_creep().without_dome_internals().without_penetrations().without_failed_vessel_outflow().without_internals_heat_loss());
        assert!(near(hc.lower_head_h, 5.3808, 0.001), "{hc:?}");
        assert!((hc.h2_25h_kg - 190.95).abs() < 0.01, "{hc:?}");
        assert!((hc.p_dw_peak_mpa - 0.53396739).abs() < 1e-8, "{hc:?}");
        // The 1000 K cutoff: the previous baseline, exactly. The ablations
        // pinned as "the model before" run on it.
        let mut old = Params::default();
        old.oxidation_min_k = 1000.0;
        let o = measure_with(base.without_hydrogen_cp_t().without_msl_creep().without_dome_internals().without_penetrations().without_failed_vessel_outflow().without_internals_heat_loss(), old);
        assert!(near(o.lower_head_h, 4.4844, 0.001), "{o:?}");
        assert!((o.h2_25h_kg - 292.44).abs() < 0.01, "{o:?}");
        // (Dry-vessel enthalpy fix: drywell 0.56407062 -> 0.56405052 MPa.)
        assert!((o.p_dw_peak_mpa - 0.56405052).abs() < 1e-8, "{o:?}");
        // ~~No vent, no explosion~~: since gh:#1036 the wetwell vent flows,
        // at ~~25.12 h~~ 23.73 h since gh:#1041 (BSAF: 23.5-24.68 h, Tier A);
        // since the 1100 K cutoff, not at all (above).
        assert!(near(o.vent_h, 23.7336, 0.001) && o.explosion_h.is_none(), "{o:?}");
        // Without the diffusion limit: the baseline before that, exactly.
        let dl = measure_with(base.without_diffusion_limit().without_hydrogen_cp_t().without_msl_creep().without_dome_internals().without_penetrations().without_failed_vessel_outflow().without_internals_heat_loss(), old);
        // (Dry-vessel enthalpy fix: 4.5089 -> 4.5075 h, 0.56293122 ->
        // 0.56289635 MPa.)
        assert!(near(dl.lower_head_h, 4.5075, 0.001), "{dl:?}");
        assert!((dl.h2_25h_kg - 287.47).abs() < 0.01, "{dl:?}");
        assert!((dl.p_dw_peak_mpa - 0.56289635).abs() < 1e-8, "{dl:?}");
        // And constant gas c_v as well: the one before that.
        let gc = measure_with(base.without_diffusion_limit().without_gas_cv_t().without_hydrogen_cp_t().without_msl_creep().without_dome_internals().without_penetrations().without_failed_vessel_outflow().without_internals_heat_loss(), old);
        // (Dry-vessel enthalpy fix: 0.56292285 -> 0.56285187 MPa.)
        assert!((gc.p_dw_peak_mpa - 0.56285187).abs() < 1e-8, "{gc:?}");
        assert_eq!((gc.lower_head_h, gc.h2_25h_kg, gc.vent_h), (dl.lower_head_h, dl.h2_25h_kg, dl.vent_h));
        // The steam-only choking ablation is the model before the mixture
        // correction, exactly.
        let mc = measure_with(base.without_mixture_choking().without_gas_cv_t().without_diffusion_limit().without_hydrogen_cp_t().without_msl_creep().without_dome_internals().without_penetrations().without_failed_vessel_outflow().without_internals_heat_loss(), old);
        assert!(near(mc.lower_head_h, 4.5092, 0.001), "{mc:?}");
        assert!((mc.h2_25h_kg - 286.63).abs() < 0.01, "{mc:?}");
        assert!((mc.p_dw_peak_mpa - 0.5622).abs() < 0.0001, "{mc:?}");
        // All three decay switches off (with steam-only choking) is the
        // decay heat before gh:#1041, so the model before it, exactly.
        let od = measure_with(
            base.without_decay_mix().without_capture_correction().without_actinide_decay().without_mixture_choking().without_gas_cv_t().without_diffusion_limit().without_hydrogen_cp_t().without_msl_creep().without_dome_internals().without_penetrations().without_failed_vessel_outflow().without_internals_heat_loss(),
            old,
        );
        assert!(near(od.lower_head_h, 4.6678, 0.001), "{od:?}");
        assert!((od.h2_25h_kg - 283.76).abs() < 0.01, "{od:?}");
        // (Dry-vessel enthalpy fix: vent 25.1231 -> 25.1256 h.)
        assert!(near(od.vent_h, 25.1256, 0.001), "{od:?}");
        // The no-swell ablation. With the ZrO2-melting collapse (in the
        // same change) it differs from the model before gh:#1036 only if a
        // node's cladding reaches 2990 K, which it does not without swell.
        // Since the 1100 K cutoff (the regime flip, above): 5.371 ->
        // 5.132 h, 190.59 -> 242.08 kg, 0.5341 -> 0.5608 MPa; it now vents.
        // Since H2 c_p(T): 5.132 -> 5.136 h, 242.08 -> 244.74 kg.
        // Since MSL creep: 5.136 -> 5.177 h, 244.74 -> 234.07 kg.
        // Since the dry-vessel outflow cap: 5.177 -> 5.169 h, 234.07 -> 233.73 kg.
        // Since the dome internals: 5.169 -> 5.439 h, 233.73 -> 211.16 kg.
        // Since penetrations: 5.439 -> 5.264 h.
        // Since failed-vessel outflow: drywell 0.5472 -> 0.5542 MPa.
        // Since internals heat loss: 5.264 -> 5.248 h, 211.16 -> 175.23 kg.
        let ns = measure(base.without_level_swell());
        assert!(near(ns.lower_head_h, 5.248, 0.001), "{ns:?}");
        assert!((ns.h2_25h_kg - 175.23).abs() < 0.01, "{ns:?}");
        assert!((ns.p_dw_peak_mpa - 0.5371).abs() < 0.0001, "{ns:?}");
        // The no-radiation ablation. ~~The model before gh:#1037, exactly
        // (5.3953 h, 216.06 kg, 0.5319 MPa).~~ Since gh:#1036 it carries
        // the swell too, so it isolates radiation. Since the diffusion
        // limit (2026-10-29) it flips into the early regime: 4.973 ->
        // 4.600 h, 245.33 -> 274.33 kg, vent at 23.96 h (the no-swell,
        // no-collapse and constant-c_p ablations did not move).
        // Since the 1100 K cutoff: 4.600 -> 4.730 h, 274.33 -> 259.20 kg.
        // Since H2 c_p(T): 4.730 -> 4.723 h, 259.20 -> 252.24 kg.
        // Since MSL creep: 4.723 -> 4.796 h, 252.24 -> 252.49 kg.
        // Since the dome internals: 4.796 -> 5.088 h, 252.49 -> 264.51 kg.
        // Since penetrations: 5.088 -> 4.903 h.
        // Since internals heat loss: 4.903 -> 5.025 h, 264.51 -> 235.69 kg.
        let nr = measure(base.without_core_radiation());
        assert!(near(nr.lower_head_h, 5.025, 0.001), "{nr:?}");
        assert!((nr.h2_25h_kg - 235.69).abs() < 0.01, "{nr:?}");
        assert!((nr.p_dw_peak_mpa - 0.5608).abs() < 0.0001, "{nr:?}");
        // The no-collapse ablation. ~~The model before gh:#1035's collapse
        // step, exactly (5.4125 h, 180.57 kg, 0.5080 MPa).~~ Since
        // gh:#1037 it carries radiation too, so it isolates the collapse.
        // Since the 1100 K cutoff: 5.396 -> 5.332 h, 134.37 -> 147.15 kg,
        // 0.5094 -> 0.5148 MPa.
        // Since H2 c_p(T): 5.332 -> 5.326 h, 147.15 -> 151.38 kg.
        // Since MSL creep: 5.326 -> 5.366 h, 151.38 -> 147.08 kg.
        // Since the dry-vessel outflow cap: 5.366 -> 5.374 h, 147.08 -> 148.02 kg.
        // Since the dry-vessel enthalpy fix: 148.02 -> 147.63 kg.
        // Since the dome internals: 5.374 -> 5.699 h, 147.63 -> 162.62 kg.
        // Since penetrations: 5.699 -> 5.590 h, 162.62 -> 161.17 kg.
        // Since failed-vessel outflow: drywell 0.5163 -> 0.5294 MPa.
        // Since internals heat loss: 5.590 -> 5.610 h, 161.17 -> 128.65 kg.
        let nc = measure(base.without_rod_collapse());
        assert!(near(nc.lower_head_h, 5.610, 0.001), "{nc:?}");
        assert!((nc.h2_25h_kg - 128.65).abs() < 0.01, "{nc:?}");
        assert!((nc.p_dw_peak_mpa - 0.5146).abs() < 0.0001, "{nc:?}");
        // The no-wall ablation. ~~The model before gh:#1034, exactly (5.399 h,
        // 120.73 kg, 0.4746 MPa).~~ Since Zr's heat of fusion (gh:#1035) it
        // carries that too, so it isolates the wall alone. Since the
        // diffusion limit: 4.331 -> 4.327 h, 286.97 -> 288.98 kg.
        // Since the 1100 K cutoff: 4.327 -> 5.054 h, 288.98 -> 190.58 kg,
        // 0.5608 -> 0.5307 MPa.
        // Since H2 c_p(T): 5.054 -> 5.054 h, 190.58 -> 192.68 kg.
        // Since MSL creep: 5.054 -> 5.166 h, 192.68 -> 191.86 kg.
        // Since the dry-vessel outflow cap: 5.166 -> 5.167 h, 191.86 -> 191.89 kg.
        // Since the dome internals: 5.167 -> 5.459 h, 191.89 -> 199.63 kg.
        // Since failed-vessel outflow: drywell 0.5334 -> 0.5460 MPa.
        // Since internals heat loss: 5.459 -> 4.569 h, 199.63 -> 297.01 kg.
        let nw = measure(base.without_lower_head_wall());
        assert!(near(nw.lower_head_h, 4.569, 0.001), "{nw:?}");
        assert!((nw.h2_25h_kg - 297.01).abs() < 0.01, "{nw:?}");
        assert!((nw.p_dw_peak_mpa - 0.5952).abs() < 0.0001, "{nw:?}");
        // The constant-c_p ablation. ~~The model before gh:#1038, exactly
        // (5.391 h, 126.62 kg, 0.4767 MPa).~~ Since the reaction-heat
        // correction it carries 6.763 MJ/kg too, so it isolates c_p alone.
        // Since gh:#1046: 5.371 -> 5.386 h, 122.96 -> 118.02 kg (H2 at 25 h
        // is not smooth in the inputs: input-sensitivity.md). Since gh:#1034
        // (the wall in both property paths): 5.386 -> 5.535 h, 118.02 ->
        // 116.27 kg. Since rod collapse (gh:#1035): 5.535 -> 5.521 h,
        // 116.27 -> 157.33 kg. Since core radiation (gh:#1037): 5.521 ->
        // 5.653 h, 157.33 -> 135.62 kg.
        // Since the 1100 K cutoff: 5.327 -> 5.344 h, 162.16 -> 159.33 kg.
        // Since H2 c_p(T): 5.344 -> 5.331 h, 159.33 -> 167.34 kg.
        // Since MSL creep: 5.331 -> 5.329 h, 167.34 -> 164.24 kg.
        // Since the dry-vessel outflow cap: 5.329 -> 5.333 h, 164.24 -> 164.83 kg.
        // Since the dry-vessel enthalpy fix: 164.83 -> 164.71 kg.
        // Since the dome internals: 5.333 -> 5.670 h, 164.71 -> 159.62 kg.
        // Since penetrations: 5.670 -> 5.493 h.
        // Since failed-vessel outflow: drywell 0.5134 -> 0.5277 MPa.
        // Since internals heat loss: 5.493 -> 5.506 h, 159.62 -> 159.56 kg.
        let cp = measure(base.without_temperature_dependent_properties());
        assert!(near(cp.lower_head_h, 5.506, 0.001), "{cp:?}");
        assert!((cp.h2_25h_kg - 159.56).abs() < 0.01, "{cp:?}");
        assert!((cp.p_dw_peak_mpa - 0.5286).abs() < 0.0001, "{cp:?}");
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
        assert_eq!(runs.len(), 64);
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
        assert!(!ablations()[12].1.gas_cv_t && ablations()[12].1.mixture_choking);
        assert!(!ablations()[13].1.diffusion_limit && ablations()[13].1.gas_cv_t);
        assert!(!ablations()[14].1.hydrogen_cp_t && ablations()[14].1.diffusion_limit);
        assert!(!ablations()[15].1.msl_creep && ablations()[15].1.hydrogen_cp_t);
        assert!(!ablations()[16].1.dome_internals && ablations()[16].1.msl_creep);
        assert!(!ablations()[17].1.penetrations && ablations()[17].1.dome_internals);
        assert!(!ablations()[18].1.failed_vessel_outflow && ablations()[18].1.penetrations);
        assert!(!ablations()[19].1.pool_scrubbing && ablations()[19].1.failed_vessel_outflow);
        assert!(!ablations()[20].1.in_vessel_deposition && ablations()[20].1.pool_scrubbing);
        assert!(!ablations()[21].1.internals_heat_loss && ablations()[21].1.in_vessel_deposition);
        assert!(!ablations()[22].1.scrubbing_saturation && ablations()[22].1.internals_heat_loss);
        assert!(!ablations()[23].1.swarm_scrubbing && ablations()[23].1.scrubbing_saturation);
        assert!(!ablations()[24].1.sectional_classes && ablations()[24].1.swarm_scrubbing);
        assert!(!ablations()[25].1.iodine_partition && ablations()[25].1.sectional_classes);
        assert!(!ablations()[26].1.mixture_vent && ablations()[26].1.iodine_partition);
        assert!(!ablations()[27].1.phoresis && ablations()[27].1.mixture_vent);
        assert!(!ablations()[28].1.iodine_swarm && ablations()[28].1.phoresis);
        assert!(!ablations()[29].1.vessel_aerosol && ablations()[29].1.iodine_swarm);
        assert!(!ablations()[30].1.condensate_mass && ablations()[30].1.vessel_aerosol);
        assert!(!ablations()[31].1.fp_decay_follows && ablations()[31].1.condensate_mass);
        assert!(!ablations()[32].1.te_transport && ablations()[32].1.fp_decay_follows);
        assert!(!ablations()[33].1.debris_oxidation && ablations()[33].1.te_transport);
        assert!(!ablations()[34].1.candling && ablations()[34].1.debris_oxidation);
        assert!(!ablations()[35].1.debris_quench && ablations()[35].1.candling);
        assert!(!ablations()[36].1.wet_head_contact && ablations()[36].1.debris_quench);
        assert!(!ablations()[37].1.penetration_coverage && ablations()[37].1.wet_head_contact);
        assert!(!ablations()[38].1.failed_vessel_drain && ablations()[38].1.penetration_coverage);
        assert!(!ablations()[39].1.ww_atmosphere && ablations()[39].1.failed_vessel_drain);
        assert!(!ablations()[40].1.dw_wall_hs && ablations()[40].1.ww_atmosphere);
        assert!(!ablations()[41].1.ww_shell && ablations()[41].1.dw_wall_hs);
        assert!(!ablations()[42].1.outflow_enthalpy && ablations()[42].1.ww_shell);
        assert!(!ablations()[43].1.relief_h2_enthalpy && ablations()[43].1.outflow_enthalpy);
        assert!(!ablations()[44].1.condensate_to_floor && ablations()[44].1.relief_h2_enthalpy);
        assert!(!ablations()[45].1.rpv_wall && ablations()[45].1.condensate_to_floor);
        assert!(!ablations()[46].1.internals_radiation && ablations()[46].1.rpv_wall);
        assert!(!ablations()[47].1.core_top_radiation && ablations()[47].1.internals_radiation);
        assert!(!ablations()[48].1.see_through && ablations()[48].1.core_top_radiation);
        assert!(!ablations()[49].1.mcci && ablations()[49].1.see_through);
        assert!(!ablations()[50].1.dw_wall_outer && ablations()[50].1.mcci);
        assert!(!ablations()[51].1.core_plate && ablations()[51].1.dw_wall_outer);
        assert!(!ablations()[52].1.bottom_radiation && ablations()[52].1.core_plate);
        assert!(!ablations()[53].1.ring_plates && ablations()[53].1.bottom_radiation);
        assert!(!ablations()[54].1.pool_layers && ablations()[54].1.ring_plates);
        assert!(!ablations()[55].1.plate_oxidation && ablations()[55].1.pool_layers);
        assert!(!ablations()[56].1.lp_steam_consumed && ablations()[56].1.plate_oxidation);
        assert!(!ablations()[57].1.control_blades && ablations()[57].1.lp_steam_consumed);
        assert!(!ablations()[58].1.blade_fusion && ablations()[58].1.control_blades);
        assert!(!ablations()[59].1.cavity_phases && ablations()[59].1.blade_fusion);
        assert!(!ablations()[60].1.bur_ignition && ablations()[60].1.cavity_phases);
        assert!(!ablations()[61].1.gas_choking && ablations()[61].1.bur_ignition);
        assert!(!ablations()[62].1.blade_diffusion_limit && ablations()[62].1.gas_choking);
        assert!(!ablations()[63].1.two_sided_equalising && ablations()[63].1.blade_diffusion_limit);
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
