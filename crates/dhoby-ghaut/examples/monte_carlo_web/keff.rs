//! What every rung's k-eigenvalue run shares: the settings a reader picks,
//! one generation as the page sees it, the `openmc.run()`-style console, and
//! the closing summary (analog counts, `k` by counting, the comparison with
//! the rung's reference). The power iteration itself is each rung's own
//! (`<rung>/sim.rs`), on outram-mc-libs.

use outram_mc_libs::physics::keff::{GenerationReport, HistoryCounts};

/// The settings a reader chooses, and where the source starts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KeffConfig {
    pub n_particles: usize,
    pub n_inactive: usize,
    pub n_active: usize,
    pub seed: u64,
    /// Start from every neutron at the centre (Watch: shows the source
    /// spreading) rather than the rung's ordinary initial source (Run).
    pub point_source: bool,
    /// Send the next generation's source sites to the page (Watch draws them).
    pub want_sites: bool,
}

/// Most neutrons per generation a reader may ask for (maintainer request
/// 2026-10-15; was 20 000).
///
/// **What the caps cost, measured 2026-10-15** on an Intel Xeon @ 2.80 GHz
/// (Cascade Lake, KVM guest, 4 vCPUs, 15 GiB RAM), native release, one
/// thread, `--headless-keff-rung lct008 100000 1 2 1` (Exact tier):
/// transport 76.5 s for 3 × 100 000 histories, **≈ 3900 neutrons/s, 25.5 s
/// per 100 000-neutron generation**; peak RSS 551.6 MB, against 552.9 MB for
/// the same run at 1000 neutrons: the peak is the data processing, and a
/// 100 000-neutron source and bank (≈ 72 B a site, ≈ 15 MB) stay under it.
/// At the caps, 100 000 × 5000 = 5e8 histories is **≈ 35 h** of LCT-008 on
/// this core natively; in a browser (wasm 1.3-2.5× slower, the
/// `htr10_full_core_web` record) ≈ 46-89 h, and on a phone core (an
/// assumed 2-4× slower again, not measured) ≈ 4-15 days. The HTR-10 core
/// (12-19 ms per history per worker in the browser, same record) is
/// thousands of worker-hours. Hence the page's measured time-left line
/// ([`eta_line`]) rather than a lower cap. What a run keeps per generation
/// on the page is a report and a console line (a few hundred bytes; ≈ 1-2
/// MB at 5000 generations).
pub const MAX_PARTICLES: usize = 100_000;

/// Most generations (inactive + active) in one run (maintainer request
/// 2026-10-15; was up to 1000 + 1000). Either part may take what the other
/// leaves, so 4998 inactive + 2 active and 1 + 4999 are both allowed.
pub const MAX_GENERATIONS: usize = 5_000;

/// Fewest active generations: one active generation has no spread.
pub const MIN_ACTIVE: usize = 2;

/// Hold `cfg` to [`MAX_PARTICLES`] and [`MAX_GENERATIONS`], with at least
/// `min_inactive` inactive and [`MIN_ACTIVE`] active generations. Inactive
/// generations are held first; the active ones get what is left.
pub fn clamp_to_caps(cfg: KeffConfig, min_inactive: usize) -> KeffConfig {
    let n_inactive = cfg.n_inactive.clamp(min_inactive, MAX_GENERATIONS - MIN_ACTIVE);
    KeffConfig {
        n_particles: cfg.n_particles.clamp(1, MAX_PARTICLES),
        n_inactive,
        n_active: cfg.n_active.clamp(MIN_ACTIVE, MAX_GENERATIONS - n_inactive),
        ..cfg
    }
}

/// `s` seconds as a reader reads it: "40 s", "12 min", "3.4 h", "2.1 days".
pub fn human_duration(s: f64) -> String {
    if !s.is_finite() || s < 0.0 {
        "unknown".to_string()
    } else if s < 90.0 {
        format!("{s:.0} s")
    } else if s < 5400.0 {
        format!("{:.0} min", s / 60.0)
    } else if s < 172_800.0 {
        format!("{:.1} h", s / 3600.0)
    } else {
        format!("{:.1} days", s / 86_400.0)
    }
}

/// A run's time left, measured on THIS device: from the generations done so
/// far and the seconds they took, the rate in neutrons per second and the
/// time the rest needs at that rate. `None` before the first generation.
/// The line says that Stop takes effect only after the generation in
/// progress (one generation is one worker request), and starts with "⚠"
/// when the rest would take more than ten minutes.
pub fn eta_line(n_particles: usize, total_gens: usize, gens_done: usize, elapsed_s: f64) -> Option<String> {
    if gens_done == 0 || elapsed_s <= 0.0 {
        return None;
    }
    let per_gen = elapsed_s / gens_done as f64;
    let left = per_gen * total_gens.saturating_sub(gens_done) as f64;
    let rate = n_particles as f64 * gens_done as f64 / elapsed_s;
    let warn = if left > 600.0 { "⚠ " } else { "" };
    Some(format!(
        "{warn}About {} left on this device ({} per generation, {rate:.0} neutrons/s measured here). Stop takes effect after the generation in progress.",
        human_duration(left),
        human_duration(per_gen)
    ))
}

/// Before a run: its size, and (when a run on this device has measured a
/// rate, neutrons per second) about how long it will take here.
pub fn plan_line(cfg: &KeffConfig, rate: Option<f64>) -> String {
    let hist = (cfg.n_particles * (cfg.n_inactive + cfg.n_active)) as f64;
    match rate.filter(|r| *r > 0.0) {
        Some(r) => {
            let t = hist / r;
            let warn = if t > 600.0 { "⚠ " } else { "" };
            format!("{warn}{hist:.2e} neutron histories: about {} here at the last run's {r:.0} neutrons/s.", human_duration(t))
        }
        None => format!("{hist:.2e} neutron histories. How long they take here is shown after the first generation."),
    }
}

/// One generation as the page sees it: the library's report plus (Watch
/// only) a sample of where the next generation's neutrons start, on x-y.
#[derive(Clone, Debug, PartialEq)]
pub struct Generation {
    pub report: GenerationReport,
    pub sites: Vec<[f32; 2]>,
}

/// At most this many source sites cross to the page per generation.
pub const MAX_SITES_SHOWN: usize = 3000;

/// Every `stride`-th of `points`, so at most [`MAX_SITES_SHOWN`] cross.
pub fn sample_sites(points: &[outram_mc_libs::geometry::position::Position]) -> Vec<[f32; 2]> {
    let stride = points.len().div_ceil(MAX_SITES_SHOWN).max(1);
    points.iter().step_by(stride).map(|p| [p.x as f32, p.y as f32]).collect()
}

/// A recorded result a reader's run is compared with, quoted from its record.
#[derive(Clone, Copy, Debug)]
pub struct Reference {
    /// What it is, with file, date and commit.
    pub label: &'static str,
    pub k: f64,
    /// Standard error of `k`.
    pub sem: f64,
    /// Seed-to-seed standard deviation of ONE run at the recorded settings.
    pub sd_one_run: f64,
    /// Active histories in one run at the recorded settings.
    pub histories_one_run: f64,
    /// A measured experiment, `(k, sigma)`, if the benchmark is one.
    pub experiment: Option<(&'static str, f64, f64)>,
    /// The same case on the reference code and data, if recorded.
    pub code: Option<CodeReference>,
}

/// A reference code's recorded `k` for a case: here route 1 of the
/// five-route study, OpenMC on NJOY2016's data.
#[derive(Clone, Copy, Debug)]
pub struct CodeReference {
    /// The codes with their versions and commits, the run size and the date.
    pub label: &'static str,
    pub k: f64,
    /// Standard error of `k`.
    pub sem: f64,
}

/// The codes of route 1, with the versions and commits the five-route record
/// states (`five_route_keff_2026_09_29.md`, "Routes").
#[macro_export]
macro_rules! route1_label {
    ($size:literal) => {
        concat!("OpenMC 0.16.1-dev25 at d7d3284a1 on NJOY2016 2016.79 at ac5adf5, five-route study route 1, ", $size, ", 2026-10-15")
    };
}

/// Route 1's recorded `k` of `case`, read from the committed campaign summary.
pub fn route1_reference(case: &str, label: &'static str) -> Option<CodeReference> {
    let (k, sem, _) = five_route_row(case, "route1")?;
    Some(CodeReference { label, k, sem })
}

/// The commit and date this page was built at, for the line beside a k the
/// reader computed here. The site build sets both (`web/monte_carlo/build.sh`);
/// a build without them says so and invents nothing.
pub fn build_stamp() -> String {
    match (option_env!("OUTRAM_PAGE_DATE"), option_env!("OUTRAM_PAGE_COMMIT")) {
        (Some(d), Some(c)) if !d.is_empty() && !c.is_empty() => format!("computed now by the code built {d} at {c}"),
        _ => "computed now by a local build, commit not recorded".to_string(),
    }
}

/// The lines under a computed `k`: the experiment, the reference code and
/// this code's own record, each with its date and commit where it has one.
pub fn reference_lines(r: &Reference) -> Vec<String> {
    let mut v = Vec::new();
    if let Some((name, k, s)) = r.experiment {
        v.push(format!(" Experiment ({name}): {k:.4} +/- {s:.4}"));
    }
    if let Some(c) = r.code {
        v.push(format!(" Reference code ({}): {:.5} +/- {:.5} ({:+.0} +/- {:.0} pcm)", c.label, c.k, c.sem, (c.k - 1.0) * 1e5, c.sem * 1e5));
    }
    v.push(format!(" Recorded ({}): {:.5} +/- {:.5} ({:+.0} +/- {:.0} pcm)", r.label, r.k, r.sem, (r.k - 1.0) * 1e5, r.sem * 1e5));
    v
}

/// The five-route study's campaign summary (four ICSBEP cases × five code and
/// data routes), read at build time so no recorded `k` is retyped here.
const FIVE_ROUTE_SUMMARY: &str =
    include_str!("../../../outram-mc-libs/verification_and_validation/icsbep/five_route_keff/data/summary_keff.csv");

/// One row of the five-route summary: `(k, standard error, seed-to-seed
/// standard deviation)` of `case` on `route` (`route1` is OpenMC on NJOY2016
/// data, `route4` outram-mc on the Rust NJOY port reading ENDF directly).
pub fn five_route_row(case: &str, route: &str) -> Option<(f64, f64, f64)> {
    let mut lines = FIVE_ROUTE_SUMMARY.lines();
    let head: Vec<&str> = lines.next()?.split(',').collect();
    let col = |n: &str| head.iter().position(|h| *h == n);
    let (cc, cr, ck, cs, cd) = (col("case")?, col("route")?, col("k_mean")?, col("k_sem")?, col("k_sd_seeds")?);
    lines.find_map(|l| {
        let f: Vec<&str> = l.split(',').collect();
        if f.get(cc) != Some(&case) || f.get(cr) != Some(&route) {
            return None;
        }
        Some((f.get(ck)?.parse().ok()?, f.get(cs)?.parse().ok()?, f.get(cd)?.parse().ok()?))
    })
}

/// A rung's [`Reference`] from the five-route study's route 4, the route this
/// demo itself runs. `histories_one_run` is the campaign's active histories
/// per seed; `experiment` is `(name, k, sigma)`.
pub fn five_route_reference(
    case: &str,
    label: &'static str,
    route1_label: &'static str,
    histories_one_run: f64,
    experiment: (&'static str, f64, f64),
) -> Option<Reference> {
    let (k, sem, sd_one_run) = five_route_row(case, "route4")?;
    Some(Reference { label, k, sem, sd_one_run, histories_one_run, experiment: Some(experiment), code: route1_reference(case, route1_label) })
}

/// The console header, as `openmc.run()` prints it for an eigenvalue run
/// with an entropy mesh.
pub const CONSOLE_HEADER: [&str; 2] = [
    " Bat./Gen.      k       Entropy         Average k",
    " =========   ========   ========   ====================",
];

/// One console line per generation, in `openmc.run()`'s layout: generation,
/// its `k`, the source entropy, and from the second active generation on the
/// running mean and its standard error.
pub fn console_line(r: &GenerationReport) -> String {
    let h = r.entropy.map_or("        ".to_string(), |h| format!("{h:8.5}"));
    let mut s = format!("{:>8}/1    {:7.5}   {h}", r.index + 1, r.k);
    if let Some((m, e)) = r.k_mean {
        if e > 0.0 {
            s.push_str(&format!("   {m:7.5} +/- {e:7.5}"));
        }
    }
    s
}

/// The closing summary: the analog counts over the active generations, `k`
/// by counting, and the result against the experiment and the record.
pub fn summary_lines(c: &HistoryCounts, production: f64, cfg: KeffConfig, k_mean: f64, k_std: f64, reference: Option<Reference>) -> Vec<String> {
    let mut v = Vec::new();
    let t = c.tracked.max(1) as f64;
    let f = c.fissions();
    let sources = (cfg.n_particles * cfg.n_active) as f64;
    let pct = |x: u64| 100.0 * x as f64 / t;
    v.push(String::new());
    // A CSG iteration tallies no history counts: say so instead of printing
    // a block of zeros.
    if c.tracked == 0 {
        v.push(" (this iteration tallies no leaked / captured / fissioned counts)".to_string());
    } else {
        v.push(format!(
            " Neutrons followed (active): {} ({} source + {} (n,xn) secondaries)",
            c.tracked,
            sources,
            c.tracked as i64 - sources as i64
        ));
        v.push(format!("   leaked     {:>9}  {:5.1} %", c.leaked, pct(c.leaked)));
        v.push(format!("   captured   {:>9}  {:5.1} %", c.captured, pct(c.captured)));
        v.push(format!("   fissioned  {:>9}  {:5.1} %", f, pct(f)));
        let [b0, b1, b2] = c.fissions_by_energy;
        v.push(format!("   fissions by incident energy: < 0.625 eV {b0}, 0.625 eV-100 keV {b1}, > 100 keV {b2}"));
        if f > 0 {
            let nu = production / f as f64;
            let p_nl = 1.0 - c.leaked as f64 / t;
            let k_inf = nu * f as f64 / (f + c.captured) as f64;
            v.push(format!("   nu-bar {nu:.4}   k_inf = nu F/(F+C) = {k_inf:.5}   P_NL = 1 - L/N = {p_nl:.5}"));
            v.push(format!("   k_inf x P_NL = {:.5}   (k = nu F / source neutrons = {:.5})", k_inf * p_nl, production / sources));
        }
    }
    v.push(String::new());
    v.push(format!(" k-effective = {k_mean:.5} +/- {k_std:.5}   ({:+.0} +/- {:.0} pcm from 1), {}", (k_mean - 1.0) * 1e5, k_std * 1e5, build_stamp()));
    if let Some(r) = reference {
        v.extend(reference_lines(&r));
    }
    v
}

// ─── A small k_inf case (gh:#549) ────────────────────────────────────────────

/// A rung's "run a small `k_inf` case at parameter `x`" request: what the
/// parameter is, its range, the run's default settings and the recorded
/// curves the reader's points are drawn against. The `lct008` rung's pitch
/// slider is the first user; any rung whose lesson sweeps one parameter of an
/// infinite lattice can offer one ([`crate::rungs::McRung::kinf_case`]).
#[derive(Clone, Debug)]
pub struct KinfCase {
    /// Heading of the view, e.g. "k∞ against pitch".
    pub title: &'static str,
    /// Name and unit of the parameter, e.g. ("pitch", "cm").
    pub param: (&'static str, &'static str),
    pub range: (f64, f64),
    pub default: f64,
    /// Discrete cases instead of a continuous parameter, `(value, name)`
    /// (e.g. kernels homogenised / resolved); empty for a slider.
    pub choices: Vec<(f64, &'static str)>,
    /// Marked values on the axis, e.g. the benchmark's own pitch.
    pub marks: Vec<(f64, &'static str)>,
    /// Neutrons and generations of one run (point_source / want_sites unused).
    pub cfg: KeffConfig,
    /// The recorded curves, quoted from their record.
    pub curves: Vec<RecordedCurve>,
    /// What the run is and is not, one bullet each.
    pub notes: Vec<&'static str>,
}

/// The workspace's plotting convention: published curves solid, ours dotted,
/// other codes (references) dashed, recorded points as markers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineStyle {
    Published,
    Ours,
    Reference,
}

/// A recorded curve: `(x, k, sigma)` points, with where they come from.
#[derive(Clone, Debug)]
pub struct RecordedCurve {
    pub label: String,
    pub style: LineStyle,
    pub points: Vec<(f64, f64, f64)>,
}

/// One generation of a running `k_inf` case, as the page sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KinfGeneration {
    /// The parameter the case runs at.
    pub param: f64,
    /// Generation index, from 0, and the run's total.
    pub index: usize,
    pub total: usize,
    pub active: bool,
    pub k: f64,
    /// Mean and standard error over the active generations so far (NaN before).
    pub mean: f64,
    pub sem: f64,
}

/// Read `(x, k, sigma)` columns, by header name, from a recorded CSV.
pub fn csv_points(csv: &str, x: &str, k: &str, s: &str) -> Vec<(f64, f64, f64)> {
    let mut lines = csv.lines();
    let head: Vec<&str> = lines.next().unwrap_or("").split(',').collect();
    let col = |n: &str| head.iter().position(|h| h.trim() == n);
    let (Some(ix), Some(ik), Some(is)) = (col(x), col(k), col(s)) else { return Vec::new() };
    lines
        .filter_map(|l| {
            let f: Vec<&str> = l.split(',').collect();
            Some((f.get(ix)?.trim().parse().ok()?, f.get(ik)?.trim().parse().ok()?, f.get(is)?.trim().parse().ok()?))
        })
        .collect()
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    fn cfg(n: usize, i: usize, a: usize) -> KeffConfig {
        KeffConfig { n_particles: n, n_inactive: i, n_active: a, seed: 1, point_source: false, want_sites: false }
    }

    /// The caps: 100 000 neutrons per generation and 5000 generations in
    /// all, inactive held first and the active ones given what is left.
    #[test]
    fn settings_are_held_to_the_caps() {
        let c = clamp_to_caps(cfg(100_000, 1000, 4000), 1);
        assert_eq!((c.n_particles, c.n_inactive, c.n_active), (100_000, 1000, 4000));
        let c = clamp_to_caps(cfg(250_000, 3000, 4000), 1);
        assert_eq!((c.n_particles, c.n_inactive, c.n_active), (MAX_PARTICLES, 3000, 2000));
        let c = clamp_to_caps(cfg(10, 9000, 9000), 1);
        assert_eq!((c.n_inactive, c.n_active), (MAX_GENERATIONS - MIN_ACTIVE, MIN_ACTIVE));
        let c = clamp_to_caps(cfg(10, 0, 9000), 0);
        assert_eq!((c.n_inactive, c.n_active), (0, MAX_GENERATIONS));
        let c = clamp_to_caps(cfg(10, 0, 0), 1);
        assert_eq!((c.n_inactive, c.n_active), (1, MIN_ACTIVE));
    }

    /// The estimate is the measured time per generation times the
    /// generations left, and warns past ten minutes.
    #[test]
    fn the_time_left_is_measured_not_assumed() {
        assert_eq!(eta_line(1000, 10, 0, 5.0), None);
        let l = eta_line(1000, 10, 2, 4.0).expect("line");
        assert!(l.starts_with("About 16 s left") && l.contains("2 s per generation") && l.contains("500 neutrons/s"), "{l}");
        let l = eta_line(100_000, 5000, 1, 60.0).expect("line");
        assert!(l.starts_with("⚠ About 3.5 days left"), "{l}");
        assert_eq!(human_duration(30.0), "30 s");
        assert_eq!(human_duration(600.0), "10 min");
        assert_eq!(human_duration(7200.0), "2.0 h");
        assert_eq!(human_duration(f64::NAN), "unknown");
        let p = plan_line(&cfg(100_000, 1000, 4000), Some(10_000.0));
        assert!(p.starts_with("⚠ 5.00e8 neutron histories: about 13.9 h"), "{p}");
        assert!(plan_line(&cfg(1000, 10, 20), None).contains("after the first generation"));
    }
}
