//! **The diagnostics report**: one zip holding `report.md`, CSV tables and
//! PNG plots (gh #941, decisions on #925, 2026-10-17).
//!
//! ```text
//! godiva.zip
//! ├── report.md                ← the front page: what ran, the results, the plots
//! ├── k.png, entropy.png, spectrum_*.png, leakage.png, *_map_*.png
//! └── data/run_summary.csv, data/spectrum_*.csv, data/*_map.csv, ...
//! ```
//!
//! - **No JSON.** Small tables appear in `report.md` twice, as a Markdown
//!   table to read and as a fenced `csv` block to copy; big ones (the
//!   1000-bin spectrum, the maps) are linked as `data/*.csv`. Plots are
//!   embedded by relative link. `report.md` is GitHub Markdown with no math
//!   (`docs/claude-md/markdown-format.md`).
//! - **Everything is made in memory** ([`DiagnosticsReport::files`],
//!   [`DiagnosticsReport::to_zip_bytes`]), so the same code serves the CLI
//!   and the browser (wasm). Writing to a path
//!   ([`DiagnosticsReport::write_zip`],
//!   [`DiagnosticsReport::write_unzipped_to_dir`]) is a thin native-only
//!   wrapper: `std::fs` compiles on `wasm32-unknown-unknown` and then fails
//!   at run time.
//! - **No new dependency**: PNGs by outram-blender's own encoder over
//!   `miniz_oxide`, the zip by [`zip`] over the same.

mod plot;
pub mod zip;

use std::fmt::Write as _;

use super::collect::{
    Ealf, Kinetics, Leakage, MCDiagnosticResult, MeshMap, NeutronBalance, RunHealth, SixFactors,
    Spectrum,
};
use super::run::KeffRun;
use crate::physics::reactor_physics::Estimate;
use plot::{heatmap, LinePlot, Series, Style, COLOURS};

/// One file of the report: a path inside the zip and its bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputFile {
    /// Path inside the archive (`report.md`, `data/spectrum.csv`, ...).
    pub path: String,
    /// Contents.
    pub bytes: Vec<u8>,
}

/// **Everything a run produced, as files**: `report.md`, CSVs and PNGs.
/// Make it with [`KeffRun::diagnostics_report`].
#[derive(Debug, Clone, PartialEq)]
pub struct DiagnosticsReport {
    files: Vec<OutputFile>,
}

impl DiagnosticsReport {
    /// The files, `report.md` first.
    pub fn files(&self) -> &[OutputFile] {
        &self.files
    }

    /// One file by path.
    pub fn file(&self, path: &str) -> Option<&OutputFile> {
        self.files.iter().find(|f| f.path == path)
    }

    /// **The zip, in memory** (CLI and browser alike).
    ///
    /// # Errors
    /// A 32-bit ZIP limit is passed (no ZIP64).
    pub fn to_zip_bytes(&self) -> Result<Vec<u8>, zip::ZipError> {
        let entries: Vec<(String, &[u8])> = self
            .files
            .iter()
            .map(|f| (f.path.clone(), f.bytes.as_slice()))
            .collect();
        zip::zip_bytes(&entries)
    }

    /// Read an archive written by [`Self::to_zip_bytes`] back into files,
    /// checking every CRC (for a browser to verify a download, and tests).
    ///
    /// # Errors
    /// Not such an archive, or a CRC mismatch.
    pub fn read_zip(bytes: &[u8]) -> Result<Vec<OutputFile>, String> {
        Ok(zip::unzip(bytes)?
            .into_iter()
            .map(|(path, bytes)| OutputFile { path, bytes })
            .collect())
    }

    /// **Write the zip** to `path` (native only), creating its folder.
    ///
    /// # Errors
    /// I/O, or a ZIP limit.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn write_zip(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        let path = path.as_ref();
        if let Some(d) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(d)?;
        }
        let bytes = self
            .to_zip_bytes()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, bytes)
    }

    /// **Write the files unzipped** under `dir` (native only; an explicit
    /// option, the zip is the default).
    ///
    /// # Errors
    /// I/O.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn write_unzipped_to_dir(&self, dir: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        for f in &self.files {
            let p = dir.as_ref().join(&f.path);
            if let Some(d) = p.parent() {
                std::fs::create_dir_all(d)?;
            }
            std::fs::write(p, &f.bytes)?;
        }
        Ok(())
    }
}

impl KeffRun {
    /// **The diagnostics report** of this run, titled "k_eff run".
    pub fn diagnostics_report(&self) -> DiagnosticsReport {
        self.diagnostics_report_titled("k_eff run")
    }

    /// The report with your own title (e.g. "Godiva").
    pub fn diagnostics_report_titled(&self, title: &str) -> DiagnosticsReport {
        Builder::new(self.keff(), title).build(self)
    }
}

// ─────────────────────────────────────────────────────────────────────────────

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// A small table: header and rows of strings.
struct Table {
    header: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl Table {
    fn new(header: &[&str]) -> Self {
        Table {
            header: header.iter().map(|s| s.to_string()).collect(),
            rows: Vec::new(),
        }
    }
    fn row(&mut self, r: Vec<String>) {
        self.rows.push(r);
    }
    fn csv(&self) -> String {
        let mut s = self
            .header
            .iter()
            .map(|h| csv_field(h))
            .collect::<Vec<_>>()
            .join(",");
        s.push('\n');
        for r in &self.rows {
            s.push_str(&r.iter().map(|c| csv_field(c)).collect::<Vec<_>>().join(","));
            s.push('\n');
        }
        s
    }
    fn markdown(&self) -> String {
        let esc = |c: &str| c.replace('|', "\\|");
        let mut s = format!(
            "| {} |\n",
            self.header
                .iter()
                .map(|h| esc(h))
                .collect::<Vec<_>>()
                .join(" | ")
        );
        s.push_str(&format!("|{}\n", "---|".repeat(self.header.len())));
        for r in &self.rows {
            s.push_str(&format!(
                "| {} |\n",
                r.iter().map(|c| esc(c)).collect::<Vec<_>>().join(" | ")
            ));
        }
        s
    }
}

fn g(x: f64) -> String {
    if x == 0.0 {
        "0".into()
    } else if x.abs() >= 1e-3 && x.abs() < 1e5 {
        format!("{x:.6}")
    } else {
        format!("{x:.6e}")
    }
}

fn est(e: &Estimate) -> [String; 2] {
    [g(e.mean), g(e.std)]
}

fn sigma_off(a: &Estimate, b: &Estimate) -> String {
    let s = (a.std * a.std + b.std * b.std).sqrt();
    if s > 0.0 {
        format!("{:+.2}", (a.mean - b.mean) / s)
    } else {
        "n/a".into()
    }
}

fn slug(s: &str) -> String {
    let mut out: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    out.trim_matches('_').to_string()
}

struct Builder {
    keff: super::run::Keff,
    title: String,
    md: String,
    files: Vec<OutputFile>,
}

impl Builder {
    fn new(keff: super::run::Keff, title: &str) -> Self {
        Builder {
            keff,
            title: title.to_string(),
            md: String::new(),
            files: Vec::new(),
        }
    }

    fn file(&mut self, path: String, bytes: Vec<u8>) {
        self.files.push(OutputFile { path, bytes });
    }

    fn table(&mut self, t: &Table, csv_path: &str) {
        self.md.push_str(&t.markdown());
        let _ = write!(self.md, "\n```csv\n{}```\n\n", t.csv());
        self.file(format!("data/{csv_path}"), t.csv().into_bytes());
    }

    fn png(&mut self, path: &str, alt: &str, img: outram_blender::csg::plot::image::ImageData) {
        let _ = writeln!(self.md, "![{alt}]({path})\n");
        self.file(path.to_string(), img.to_png_bytes());
    }

    fn build(mut self, r: &KeffRun) -> DiagnosticsReport {
        let p = &r.provenance;
        let _ = writeln!(self.md, "# {} — k_eff run\n", self.title);
        self.md.push_str(
            "> **Research, education and V&V only** (`RESPONSIBLE_USE.md`). Not for reactor operation, \
             licensing, safety decisions or any operational use. Numbers made by outram-mc-libs, \
             AI-assisted code, untrusted until reviewed.\n\n",
        );
        let k = r.keff();
        let mut t = Table::new(&["quantity", "value", "uncertainty", "unit", "note"]);
        t.row(vec![
            "k_eff".into(),
            format!("{:.5}", k.mean),
            format!("{:.5}", k.std),
            "".into(),
            "s/sqrt(n) over active cycles".into(),
        ]);
        if let Some(s) = p.sigma_batch_means {
            t.row(vec![
                "k_eff sigma, batch means".into(),
                format!("{s:.5}"),
                "".into(),
                "".into(),
                "correlation-aware".into(),
            ]);
        }
        t.row(vec![
            "settings".into(),
            format!("{:?}", p.settings),
            "".into(),
            "".into(),
            "".into(),
        ]);
        t.row(vec![
            "seed".into(),
            p.seed.to_string(),
            "".into(),
            "".into(),
            "".into(),
        ]);
        t.row(vec![
            "neutrons per inactive cycle (start)".into(),
            p.inactive_particles.to_string(),
            "".into(),
            "".into(),
            "".into(),
        ]);
        t.row(vec![
            "neutrons per active cycle".into(),
            p.active_particles.to_string(),
            "".into(),
            "".into(),
            "".into(),
        ]);
        t.row(vec![
            "inactive cycles".into(),
            p.inactive_cycles.to_string(),
            "".into(),
            "".into(),
            "chosen by the watcher for a preset".into(),
        ]);
        t.row(vec![
            "active cycles".into(),
            p.active_cycles.to_string(),
            "".into(),
            "".into(),
            "".into(),
        ]);
        t.row(vec![
            "source converged".into(),
            if p.convergence.converged() {
                "yes"
            } else {
                "NO"
            }
            .into(),
            "".into(),
            "".into(),
            format!("{:?}", p.convergence),
        ]);
        t.row(vec![
            "data library".into(),
            p.data_library.clone(),
            "".into(),
            "".into(),
            "".into(),
        ]);
        t.row(vec![
            "temperature".into(),
            format!("{}", p.temperature_k),
            "".into(),
            "K".into(),
            "".into(),
        ]);
        t.row(vec![
            "backend".into(),
            p.backend.to_string(),
            "".into(),
            "".into(),
            "".into(),
        ]);
        for n in &p.physics {
            t.row(vec![
                format!("physics {}", n.nuclide),
                format!(
                    "URR {}, DBRC {}, thermal {}",
                    if n.urr_probability_tables {
                        "on"
                    } else {
                        "none"
                    },
                    if n.dbrc.is_ok() { "on" } else { "OFF" },
                    n.thermal_law.as_deref().unwrap_or("free gas")
                ),
                "".into(),
                "".into(),
                n.dbrc.clone().err().unwrap_or_default(),
            ]);
        }
        for a in &p.ablations {
            t.row(vec![
                "ABLATED".into(),
                a.clone(),
                "".into(),
                "".into(),
                "advanced".into(),
            ]);
        }
        for n in &p.notes {
            t.row(vec![
                "note".into(),
                n.clone(),
                "".into(),
                "".into(),
                "".into(),
            ]);
        }
        self.md.push_str("## Run\n\n");
        self.table(&t, "run_summary.csv");
        let results: Vec<MCDiagnosticResult> =
            r.diagnostics.iter().map(|(_, x)| x.clone()).collect();
        if results.is_empty() {
            self.md
                .push_str("No diagnostics were requested (`KeffSettings::new(.., None)`).\n");
        }
        let mut spectra: Vec<Spectrum> = Vec::new();
        for (i, res) in results.iter().enumerate() {
            match res {
                MCDiagnosticResult::RunHealth(h) => self.run_health(h),
                MCDiagnosticResult::NeutronBalance(b) => self.balance(b),
                MCDiagnosticResult::Ealf(e) => self.ealf(e),
                MCDiagnosticResult::Leakage(l) => self.leakage(l, i),
                MCDiagnosticResult::SixFactors(s) => self.six_factors(s, i),
                MCDiagnosticResult::Spectrum(s) => {
                    self.spectrum(s, i);
                    spectra.push(s.clone());
                }
                MCDiagnosticResult::Kinetics(k) => self.kinetics(k),
                MCDiagnosticResult::FissionMap(m) => self.map(m, "fission_map", i),
                MCDiagnosticResult::FluxMap(m) => self.map(m, "flux_map", i),
            }
        }
        if spectra.len() > 1 {
            self.md.push_str("## All spectra\n\n");
            let series = spectra
                .iter()
                .enumerate()
                .map(|(j, s)| spectrum_series(s, j))
                .collect();
            let plot = LinePlot {
                title: "FLUX PER UNIT LETHARGY, EVERY REGION".into(),
                x_label: "ENERGY [EV]".into(),
                y_label: "PHI PER LETHARGY (UNIT AREA)".into(),
                x_log: true,
                series,
            };
            self.png("spectra.png", "every spectrum", plot.render(1000, 640));
        }
        let mut files = vec![OutputFile {
            path: "report.md".into(),
            bytes: self.md.into_bytes(),
        }];
        files.append(&mut self.files);
        DiagnosticsReport { files }
    }

    fn run_health(&mut self, h: &RunHealth) {
        self.md.push_str("## Run health\n\n");
        let mut t = Table::new(&["quantity", "value", "note"]);
        t.row(vec![
            "source histories".into(),
            h.histories.to_string(),
            "all cycles".into(),
        ]);
        t.row(vec![
            "collisions per history".into(),
            format!("{:.3}", h.collisions_per_history),
            "".into(),
        ]);
        t.row(vec![
            "lost (no cell found)".into(),
            h.lost_locate.to_string(),
            "should be 0".into(),
        ]);
        t.row(vec![
            "stuck (event cap)".into(),
            h.stuck_events.to_string(),
            "should be 0".into(),
        ]);
        t.row(vec![
            "escaped with no surface ahead".into(),
            h.leak_infinity.to_string(),
            "should be 0".into(),
        ]);
        t.row(vec![
            "negative boundary distances".into(),
            h.negative_distances.to_string(),
            "should be 0".into(),
        ]);
        t.row(vec![
            "all counters clean".into(),
            if h.clean() { "yes" } else { "NO" }.into(),
            "".into(),
        ]);
        t.row(vec![
            "source converged".into(),
            if h.converged { "yes" } else { "NO" }.into(),
            format!("{} inactive cycles", h.inactive_cycles),
        ]);
        t.row(vec![
            "transport wall time [s]".into(),
            h.transport_seconds
                .map_or("not measured (wasm)".into(), |s| format!("{s:.1}")),
            format!("{} thread(s); {}", h.threads, h.hardware),
        ]);
        self.table(&t, "run_health.csv");
        let mut c = Table::new(&["cycle", "active", "particles", "k", "entropy_bits"]);
        for (i, kk) in h.k_by_cycle.iter().enumerate() {
            c.row(vec![
                (i + 1).to_string(),
                (i >= h.inactive_cycles).to_string(),
                h.particles_by_cycle
                    .get(i)
                    .map_or(String::new(), |n| n.to_string()),
                format!("{kk:.6}"),
                h.entropy_by_cycle
                    .get(i)
                    .copied()
                    .flatten()
                    .map_or(String::new(), |e| format!("{e:.6}")),
            ]);
        }
        let _ = writeln!(
            self.md,
            "Per cycle: [`data/k_by_cycle.csv`](data/k_by_cycle.csv).\n"
        );
        self.file("data/k_by_cycle.csv".into(), c.csv().into_bytes());
        let pts = |v: Vec<(f64, f64)>, label: &str| Series {
            label: label.into(),
            style: Style::Ours,
            colour: COLOURS[0],
            points: v,
        };
        let kp = LinePlot {
            title: format!("K PER CYCLE ({} INACTIVE)", h.inactive_cycles),
            x_label: "CYCLE".into(),
            y_label: "K".into(),
            x_log: false,
            series: vec![pts(
                h.k_by_cycle
                    .iter()
                    .enumerate()
                    .map(|(i, k)| ((i + 1) as f64, *k))
                    .collect(),
                "K (OUTRAM-MC, THIS RUN)",
            )],
        };
        self.png("k.png", "k per cycle", kp.render(900, 520));
        let ep = LinePlot {
            title: "SOURCE ENTROPY PER CYCLE".into(),
            x_label: "CYCLE".into(),
            y_label: "ENTROPY [BITS]".into(),
            x_log: false,
            series: vec![pts(
                h.entropy_by_cycle
                    .iter()
                    .enumerate()
                    .filter_map(|(i, e)| e.map(|e| ((i + 1) as f64, e)))
                    .collect(),
                "ENTROPY (OUTRAM-MC, THIS RUN)",
            )],
        };
        self.png(
            "entropy.png",
            "source entropy per cycle",
            ep.render(900, 520),
        );
    }

    fn balance(&mut self, b: &NeutronBalance) {
        self.md
            .push_str("## Neutron balance (per source neutron)\n\n");
        let mut t = Table::new(&[
            "row",
            "absorption",
            "sigma",
            "fission",
            "sigma",
            "capture",
            "sigma",
            "nu-fission",
            "sigma",
            "flux [cm]",
            "sigma",
        ]);
        for row in b.rows.iter().chain(std::iter::once(&b.total)) {
            let mut v = vec![row.label.clone()];
            for e in [
                &row.absorption,
                &row.fission,
                &row.capture,
                &row.production,
                &row.flux,
            ] {
                v.extend(est(e));
            }
            t.row(v);
        }
        self.table(&t, "neutron_balance.csv");
        let mut c = Table::new(&["check", "value", "sigma", "note"]);
        let [lm, ls] = est(&b.leakage);
        c.row(vec![
            "leakage".into(),
            lm,
            ls,
            "escapes per source neutron".into(),
        ]);
        for (name, e, note) in [
            (
                "analog fission",
                &b.analog_fission,
                "neutrons ended by fission",
            ),
            ("analog capture", &b.analog_capture, "ended by capture"),
            ("analog leakage", &b.analog_leakage, "ended by leaking"),
            (
                "ends per source neutron",
                &b.ends_per_source,
                "1 + net (n,xn) gain",
            ),
            (
                "closure",
                &b.closure,
                "(absorption + leakage) / ends - 1; 0 within sigma when the balance closes",
            ),
        ] {
            let [m, s] = est(e);
            c.row(vec![name.into(), m, s, note.into()]);
        }
        let z = if b.closure.std > 0.0 {
            b.closure.mean / b.closure.std
        } else {
            f64::NAN
        };
        c.row(vec![
            "closure in sigma".into(),
            format!("{z:+.2}"),
            "".into(),
            "".into(),
        ]);
        self.table(&c, "neutron_balance_check.csv");
    }

    fn ealf(&mut self, e: &Ealf) {
        self.md.push_str("## EALF\n\n");
        let mut t = Table::new(&["quantity", "value", "sigma", "unit", "note"]);
        let [m, s] = est(&e.ealf_ev);
        t.row(vec![
            "EALF".into(),
            m,
            s,
            "eV".into(),
            "energy of the average lethargy causing fission".into(),
        ]);
        t.row(vec![
            "binning bound".into(),
            format!("{:.4}", e.discretisation_bound_ln),
            "".into(),
            "in ln E".into(),
            format!(
                "|ln EALF error| from bin midpoints <= this ({:.2} %)",
                100.0 * (e.discretisation_bound_ln.exp() - 1.0)
            ),
        ]);
        t.row(vec![
            "fission outside grid".into(),
            g(e.fission_outside_grid),
            "".into(),
            "fraction".into(),
            "not counted".into(),
        ]);
        self.table(&t, "ealf.csv");
    }

    fn leakage(&mut self, l: &Leakage, i: usize) {
        self.md.push_str("## Leakage\n\n");
        let mut t = Table::new(&["quantity", "value", "sigma", "note"]);
        let [a, b] = est(&l.per_source);
        t.row(vec!["leakage per source neutron".into(), a, b, "".into()]);
        let [a, b] = est(&l.fraction_of_ends);
        t.row(vec![
            "leaked / ended".into(),
            a,
            b,
            "leakage probability of a neutron".into(),
        ]);
        t.row(vec![
            "below / above grid".into(),
            format!("{} / {}", g(l.outside_grid.0), g(l.outside_grid.1)),
            "".into(),
            "per source neutron".into(),
        ]);
        self.table(&t, &format!("leakage_{i}.csv"));
        let mut c = Table::new(&["e_lo_ev", "e_hi_ev", "leakage_per_source", "sigma"]);
        for (k, e) in l.per_bin.iter().enumerate() {
            let [m, s] = est(e);
            c.row(vec![g(l.edges_ev[k]), g(l.edges_ev[k + 1]), m, s]);
        }
        let path = format!("data/leakage_spectrum_{i}.csv");
        let _ = writeln!(self.md, "Spectrum: [`{path}`]({path}).\n");
        self.file(path, c.csv().into_bytes());
        let pts = l
            .per_bin
            .iter()
            .zip(l.edges_ev.windows(2))
            .flat_map(|(e, w)| {
                let du = (w[1] / w[0]).ln();
                let v = if du > 0.0 && du.is_finite() {
                    e.mean / du
                } else {
                    f64::NAN
                };
                [(w[0], v), (w[1], v)]
            })
            .collect();
        let plot = LinePlot {
            title: "LEAKAGE PER UNIT LETHARGY".into(),
            x_label: "ENERGY [EV]".into(),
            y_label: "LEAKAGE PER SOURCE NEUTRON PER LETHARGY".into(),
            x_log: true,
            series: vec![Series {
                label: "OUTRAM-MC, THIS RUN".into(),
                style: Style::Ours,
                colour: COLOURS[1],
                points: pts,
            }],
        };
        self.png(
            &format!("leakage_{i}.png"),
            "leakage spectrum",
            plot.render(1000, 640),
        );
    }

    fn six_factors(&mut self, s: &SixFactors, i: usize) {
        let _ = writeln!(
            self.md,
            "## Six factors (edges {} eV and {} eV)\n",
            g(s.edges_ev.0),
            g(s.edges_ev.1)
        );
        if let Some(why) = &s.undefined_because {
            let _ = writeln!(self.md, "**The six factors are undefined here:** {why}.\n");
        }
        let mut t = Table::new(&["factor", "value", "sigma", "meaning"]);
        for (n, e, m) in [
            (
                "eta",
                &s.eta,
                "thermal nu-fission / thermal absorption in fuel",
            ),
            ("f", &s.f, "thermal absorption in fuel / thermal absorption"),
            (
                "p",
                &s.p,
                "resonance escape (three-group, leakage-corrected)",
            ),
            ("epsilon", &s.epsilon, "fast fission factor"),
            ("P_FNL", &s.p_fnl, "fast + resonance non-leakage"),
            ("P_TNL", &s.p_tnl, "thermal non-leakage"),
            ("product", &s.k_from_factors, "of the six factors"),
            (
                "P / (A + L)",
                &s.production_over_losses,
                "what the product telescopes to",
            ),
            (
                "P (track-length k)",
                &s.k_tracklength,
                "nu-fission per source neutron",
            ),
        ] {
            let [a, b] = est(e);
            t.row(vec![n.into(), a, b, m.into()]);
        }
        let k = self.keff;
        let kk = Estimate {
            mean: k.mean,
            std: k.std,
        };
        t.row(vec![
            "k_eff (power iteration)".into(),
            format!("{:.6}", k.mean),
            format!("{:.6}", k.std),
            "collision estimate".into(),
        ]);
        t.row(vec![
            "P vs k_eff [sigma]".into(),
            sigma_off(&s.k_tracklength, &kk),
            "".into(),
            "two estimators of one k; should agree".into(),
        ]);
        t.row(vec![
            "product vs k_eff [sigma]".into(),
            sigma_off(&s.k_from_factors, &kk),
            "".into(),
            "below k by the (n,xn) gain in A + L".into(),
        ]);
        self.table(&t, &format!("six_factors_{i}.csv"));
        let mut gt = Table::new(&[
            "group",
            "absorption",
            "sigma",
            "nu-fission",
            "sigma",
            "leakage",
            "sigma",
        ]);
        for (j, n) in ["thermal", "resonance", "fast"].iter().enumerate() {
            let mut v = vec![n.to_string()];
            v.extend(est(&s.absorption[j]));
            v.extend(est(&s.production[j]));
            v.extend(est(&s.leakage[j]));
            gt.row(v);
        }
        let mut v = vec!["thermal, fuel".to_string()];
        v.extend(est(&s.thermal_absorption_fuel));
        v.extend(["".into(), "".into(), "".into(), "".into()]);
        gt.row(v);
        self.table(&gt, &format!("six_factor_groups_{i}.csv"));
    }

    fn spectrum(&mut self, s: &Spectrum, i: usize) {
        let _ = writeln!(self.md, "## Spectrum: {}\n", s.label);
        let mut t = Table::new(&["quantity", "value", "sigma", "note"]);
        let [a, b] = est(&s.total_in_grid);
        t.row(vec![
            "flux in grid [cm per source neutron]".into(),
            a,
            b,
            "volume-integrated track length".into(),
        ]);
        t.row(vec![
            "below / above grid".into(),
            format!("{} / {}", g(s.outside_grid.0), g(s.outside_grid.1)),
            "".into(),
            "not in the normalisation".into(),
        ]);
        let integral: f64 = s
            .per_lethargy
            .iter()
            .zip(s.edges_ev.windows(2))
            .map(|(e, w)| e.mean * (w[1] / w[0]).ln())
            .sum();
        t.row(vec![
            "integral of normalised spectrum".into(),
            format!("{integral:.12}"),
            "".into(),
            "1 by construction".into(),
        ]);
        let name = format!("spectrum_{}_{}", i, slug(&s.label));
        self.table(&t, &format!("{name}_summary.csv"));
        let mut c = Table::new(&[
            "e_lo_ev",
            "e_hi_ev",
            "flux_per_source_cm",
            "sigma",
            "flux_per_lethargy_normalised",
            "sigma",
        ]);
        for (k, (f, p)) in s.flux.iter().zip(&s.per_lethargy).enumerate() {
            let [a, b] = est(f);
            let [c1, d] = est(p);
            c.row(vec![g(s.edges_ev[k]), g(s.edges_ev[k + 1]), a, b, c1, d]);
        }
        let path = format!("data/{name}.csv");
        let _ = writeln!(self.md, "Bins: [`{path}`]({path}).\n");
        self.file(path, c.csv().into_bytes());
        let plot = LinePlot {
            title: format!("FLUX PER UNIT LETHARGY: {}", s.label.to_uppercase()),
            x_label: "ENERGY [EV]".into(),
            y_label: "PHI PER LETHARGY (UNIT AREA)".into(),
            x_log: true,
            series: vec![spectrum_series(s, 0)],
        };
        self.png(
            &format!("{name}.png"),
            &format!("spectrum, {}", s.label),
            plot.render(1000, 640),
        );
    }

    fn kinetics(&mut self, k: &Kinetics) {
        self.md.push_str("## Kinetics (IFP)\n\n");
        let mut t = Table::new(&["quantity", "value", "sigma", "unit", "note"]);
        let [a, b] = est(&k.beta_eff);
        t.row(vec![
            "beta_eff".into(),
            a,
            b,
            "".into(),
            "adjoint-weighted, iterated fission probability".into(),
        ]);
        let [a, b] = est(&k.generation_time_s);
        t.row(vec![
            "Lambda".into(),
            a,
            b,
            "s".into(),
            "time numerator / (denominator x k), as OpenMC".into(),
        ]);
        t.row(vec![
            "IFP generations".into(),
            k.ifp_generations.to_string(),
            "".into(),
            "".into(),
            "".into(),
        ]);
        t.row(vec![
            "scoring cycles".into(),
            k.scoring_cycles.to_string(),
            "".into(),
            "".into(),
            "active cycles with deep-enough lineage".into(),
        ]);
        self.table(&t, "kinetics.csv");
    }

    fn map(&mut self, m: &MeshMap, kind: &str, i: usize) {
        let _ = writeln!(
            self.md,
            "## {} ({} x {} x {} cells)\n",
            if kind == "flux_map" {
                "Flux map"
            } else {
                "Fission map"
            },
            m.mesh.dimension[0],
            m.mesh.dimension[1],
            m.mesh.dimension[2]
        );
        let _ = writeln!(
            self.md,
            "Cell size {:.4} x {:.4} x {:.4} cm; values are {}. Track-length segments are binned at their midpoint on a regular mesh (this crate's documented approximation, `tally::filter::MeshFilter`), which smears the map over about a segment length.\n",
            m.cell_cm[0], m.cell_cm[1], m.cell_cm[2], m.quantity
        );
        let [nx, ny, nz] = m.mesh.dimension;
        let groups: Vec<String> = if m.values.len() == 1 {
            vec!["all".into()]
        } else {
            let short = |x: f64| {
                if x == f64::MAX {
                    "inf".to_string()
                } else {
                    format!("{x}")
                }
            };
            let mut v: Vec<String> = m
                .group_edges_ev
                .windows(2)
                .map(|w| format!("{}-{} eV", short(w[0]), short(w[1])))
                .collect();
            v.push("total".into());
            v
        };
        let mut header = vec![
            "ix".to_string(),
            "iy".into(),
            "iz".into(),
            "x_cm".into(),
            "y_cm".into(),
            "z_cm".into(),
        ];
        for gname in &groups {
            header.push(format!("{gname}"));
            header.push(format!("{gname} sigma"));
        }
        let mut csv = header
            .iter()
            .map(|h| csv_field(h))
            .collect::<Vec<_>>()
            .join(",");
        csv.push('\n');
        let w = m.cell_cm;
        let lo = m.mesh.lower_left;
        for iz in 0..nz {
            for iy in 0..ny {
                for ix in 0..nx {
                    let c = (iz * ny + iy) * nx + ix;
                    let mut row = vec![
                        ix.to_string(),
                        iy.to_string(),
                        iz.to_string(),
                        g(lo[0] + (ix as f64 + 0.5) * w[0]),
                        g(lo[1] + (iy as f64 + 0.5) * w[1]),
                        g(lo[2] + (iz as f64 + 0.5) * w[2]),
                    ];
                    for v in &m.values {
                        row.extend(est(&v[c]));
                    }
                    csv.push_str(&row.join(","));
                    csv.push('\n');
                }
            }
        }
        let path = format!("data/{kind}_{i}.csv");
        let _ = writeln!(self.md, "Cells: [`{path}`]({path}).\n");
        self.file(path, csv.into_bytes());
        let up = m.mesh.upper_right;
        let last = m.values.len() - 1;
        let which: Vec<usize> = if m.values.len() == 1 {
            vec![0]
        } else {
            (0..m.values.len()).collect()
        };
        for gi in which {
            let gname = slug(&groups[gi]);
            let vals = &m.values[gi];
            let iz = nz / 2;
            let xy: Vec<Vec<f64>> = (0..ny)
                .map(|iy| {
                    (0..nx)
                        .map(|ix| vals[(iz * ny + iy) * nx + ix].mean)
                        .collect()
                })
                .collect();
            let label = groups[gi].to_uppercase();
            let z_mid = lo[2] + (iz as f64 + 0.5) * w[2];
            self.png(
                &format!("{kind}_{i}_{gname}_xy.png"),
                &format!("{kind} {} x-y", groups[gi]),
                heatmap(
                    &xy,
                    [lo[0], up[0], lo[1], up[1]],
                    &format!(
                        "{} {label}, X-Y AT Z = {z_mid:.2} CM",
                        kind.replace('_', " ").to_uppercase()
                    ),
                    ("X", "Y"),
                    "PER CM3",
                ),
            );
            if gi == last {
                let iy = ny / 2;
                let xz: Vec<Vec<f64>> = (0..nz)
                    .map(|iz| {
                        (0..nx)
                            .map(|ix| vals[(iz * ny + iy) * nx + ix].mean)
                            .collect()
                    })
                    .collect();
                let y_mid = lo[1] + (iy as f64 + 0.5) * w[1];
                self.png(
                    &format!("{kind}_{i}_{gname}_xz.png"),
                    &format!("{kind} {} x-z", groups[gi]),
                    heatmap(
                        &xz,
                        [lo[0], up[0], lo[2], up[2]],
                        &format!(
                            "{} {label}, X-Z AT Y = {y_mid:.2} CM",
                            kind.replace('_', " ").to_uppercase()
                        ),
                        ("X", "Z"),
                        "PER CM3",
                    ),
                );
            }
        }
    }
}

fn spectrum_series(s: &Spectrum, j: usize) -> Series {
    Series {
        label: format!("{} (OUTRAM-MC, THIS RUN)", s.label.to_uppercase()),
        style: Style::Ours,
        colour: COLOURS[j % COLOURS.len()],
        points: s
            .per_lethargy
            .iter()
            .zip(s.edges_ev.windows(2))
            .flat_map(|(e, w)| [(w[0], e.mean), (w[1], e.mean)])
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CSV quoting, Markdown escaping, number formatting and slugs.
    #[test]
    fn table_helpers() {
        let mut t = Table::new(&["a", "b"]);
        t.row(vec!["x,y".into(), "p|q".into()]);
        assert_eq!(t.csv(), "a,b\n\"x,y\",p|q\n");
        assert!(t.markdown().contains("p\\|q"));
        assert_eq!(g(0.0), "0");
        assert_eq!(g(1.5), "1.500000");
        assert_eq!(g(2.0e7), "2.000000e7");
        assert_eq!(slug("Fuel solution (A)"), "fuel_solution_a");
        let a = Estimate {
            mean: 1.0,
            std: 0.03,
        };
        let b = Estimate {
            mean: 1.0,
            std: 0.04,
        };
        assert_eq!(sigma_off(&a, &b), "+0.00");
        assert_eq!(
            sigma_off(&Estimate::exact(1.0), &Estimate::exact(1.0)),
            "n/a"
        );
    }
}
