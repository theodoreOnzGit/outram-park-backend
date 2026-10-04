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
    v.push(String::new());
    v.push(format!(" k-effective = {k_mean:.5} +/- {k_std:.5}   ({:+.0} +/- {:.0} pcm from 1)", (k_mean - 1.0) * 1e5, k_std * 1e5));
    if let Some(r) = reference {
        if let Some((name, k, s)) = r.experiment {
            v.push(format!(" Experiment ({name}): {k:.4} +/- {s:.4}"));
        }
        v.push(format!(" Recorded ({}): {:.5} +/- {:.5} ({:+.0} +/- {:.0} pcm)", r.label, r.k, r.sem, (r.k - 1.0) * 1e5, r.sem * 1e5));
    }
    v
}
