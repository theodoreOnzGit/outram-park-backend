//! The gh:#1178 study: the CFD over the design grid, and the 1-D model at
//! the same points, uncalibrated and with the closures.
use std::fmt::Write as _;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use uom::si::pressure::pascal;
use uom::si::power::watt;
use uom::si::ratio::ratio;
use uom::si::thermal_conductance::watt_per_kelvin;

use super::cfd::{self, CfdMesh, CfdRecord};
use super::closure::{ClosureError, ClosureTable, Closures};
use super::dataset::DatasetRow;
use super::design::{DesignPoint, Split};
use super::one_d::{self, eps_ntu, OneDError};
use super::{DoublePipeOutput, Theta};

/// Run the CFD at every point on `threads` threads (each run is
/// single-threaded and deterministic, so the result does not depend on the
/// thread count). Results in input order; `progress` gets one line per
/// finished point.
///
/// # Errors
/// The first CFD failure, with its point id.
pub fn run_cfd_design(points: &[DesignPoint], mesh: CfdMesh, threads: usize, progress: fn(&str)) -> Result<Vec<(String, Split, CfdRecord)>, String> {
    let next = AtomicUsize::new(0);
    let out: Mutex<Vec<Option<Result<CfdRecord, String>>>> = Mutex::new(vec![None; points.len()]);
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                if i >= points.len() {
                    break;
                }
                let p = &points[i];
                let r = cfd::run(&p.input, mesh).map_err(|e| format!("{}: {e}", p.id));
                if let Ok(rec) = &r {
                    progress(&format!("{} {:?} cells {:?} flow {} energy {} {:.1} s", p.id, p.input.to_si(), rec.cells, rec.flow_steps, rec.energy_passes, rec.wall_seconds));
                }
                if let Ok(mut g) = out.lock() {
                    g[i] = Some(r);
                }
            });
        }
    });
    let v = out.into_inner().map_err(|e| e.to_string())?;
    v.into_iter().zip(points).map(|(r, p)| r.unwrap_or_else(|| Err(format!("{}: not run", p.id))).map(|rec| (p.id.clone(), p.split, rec))).collect()
}

/// One point of the comparison.
#[derive(Debug, Clone, PartialEq)]
pub struct Comparison {
    /// The dataset row (x, split, CFD outputs).
    pub row: DatasetRow,
    /// (a) the uncalibrated 1-D model.
    pub uncalibrated: DoublePipeOutput,
    /// (b) the 1-D model with the closures: the table value at a training
    /// point, interpolated at a held-out one.
    pub with_closures: DoublePipeOutput,
    /// The closures used in (b).
    pub closures_used: Closures,
    /// The point's own closures, read off its CFD (at a held-out point,
    /// the check on the interpolation).
    pub closures_own: Closures,
}

/// Build the closure table from the training rows, and run (a) and (b) at
/// every row, on `threads` threads (each 1-D run is deterministic, so the
/// result does not depend on the thread count). Results in row order.
///
/// # Errors
/// The table (not a factorial) or the first 1-D failure.
pub fn compare(rows: &[DatasetRow], threads: usize) -> Result<Vec<Comparison>, String> {
    let all: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    compare_only(rows, &all, threads)
}

/// [`compare`] at the rows whose id is in `ids` only; the closure table is
/// still built from every training row of `rows`.
///
/// # Errors
/// As [`compare`].
pub fn compare_only(rows: &[DatasetRow], ids: &[&str], threads: usize) -> Result<Vec<Comparison>, String> {
    let train: Vec<_> = rows.iter().filter(|r| r.split == Split::Training).map(|r| (r.split, r.input, Closures::from_cfd(&r.input, &r.output))).collect();
    let table = ClosureTable::build(&train).map_err(|e: ClosureError| e.to_string())?;
    let one = |r: &DatasetRow| -> Result<Comparison, String> {
        let own = Closures::from_cfd(&r.input, &r.output);
        let used = match r.split {
            Split::Training => own,
            Split::HeldOut => table.at(&r.input).map_err(|e| format!("{}: {e}", r.id))?,
        };
        let unc = one_d::try_evaluate(&r.input, &Theta::nominal()).map_err(|e: OneDError| format!("{}: {e}", r.id))?;
        let clo = one_d::evaluate_with_closures(&r.input, &used).map_err(|e| format!("{}: {e}", r.id))?;
        Ok(Comparison { row: r.clone(), uncalibrated: unc, with_closures: clo, closures_used: used, closures_own: own })
    };
    let rows: Vec<&DatasetRow> = rows.iter().filter(|r| ids.contains(&r.id.as_str())).collect();
    let next = AtomicUsize::new(0);
    let out: Mutex<Vec<Option<Result<Comparison, String>>>> = Mutex::new(vec![None; rows.len()]);
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                if i >= rows.len() {
                    break;
                }
                let r = one(rows[i]);
                if let Ok(mut g) = out.lock() {
                    g[i] = Some(r);
                }
            });
        }
    });
    let v = out.into_inner().map_err(|e| e.to_string())?;
    v.into_iter().zip(rows).map(|(r, row)| r.unwrap_or_else(|| Err(format!("{}: not run", row.id)))).collect()
}

/// Relative difference `a/b − 1`.
#[must_use]
pub fn rel(a: f64, b: f64) -> f64 {
    a / b - 1.0
}

/// The 1-D results CSV (SI, units in the headers; relative errors are 1-D
/// against the CFD).
#[must_use]
pub fn comparison_csv(c: &[Comparison]) -> String {
    let mut s = String::new();
    for l in super::dataset::PROVENANCE.lines() {
        let _ = writeln!(s, "# {l}");
    }
    s.push_str("# This file: the TUAS 1-D model at the dataset's points, (a) uncalibrated, (b) with the CFD closures (interpolated at held_out rows).\n");
    s.push_str("id,split,ua_cfd [W/K],q_cfd [W],eps_cfd [-],eps_ntu_cfd [-],ua_a [W/K],q_a [W],eps_a [-],eps_ntu_a [-],dp_hot_a [Pa],dp_cold_a [Pa],nu_hot_a [-],nu_cold_a [-],ua_b [W/K],q_b [W],eps_b [-],eps_ntu_b [-],dp_hot_b [Pa],dp_cold_b [Pa],nu_hot_b [-],nu_cold_b [-],nu_hot_cfd [-],nu_cold_cfd [-],loss_hot_b [-],loss_cold_b [-],loss_hot_cfd [-],loss_cold_cfd [-]\n");
    for k in c {
        let (y, a, b) = (&k.row.output, &k.uncalibrated, &k.with_closures);
        let x = &k.row.input;
        let f = |o: &DoublePipeOutput| {
            [o.ua.get::<watt_per_kelvin>(), o.q.get::<watt>(), o.effectiveness.get::<ratio>(), eps_ntu(x, o.ua), o.dp_hot.get::<pascal>(), o.dp_cold.get::<pascal>(), o.nu_hot.get::<ratio>(), o.nu_cold.get::<ratio>()]
        };
        let fy = f(y);
        let mut v: Vec<String> = vec![k.row.id.clone(), k.row.split.as_str().into()];
        v.extend(fy[..4].iter().map(f64::to_string));
        v.extend(f(a).iter().map(f64::to_string));
        v.extend(f(b).iter().map(f64::to_string));
        v.push(fy[6].to_string());
        v.push(fy[7].to_string());
        for r in [k.closures_used.loss_hot, k.closures_used.loss_cold, k.closures_own.loss_hot, k.closures_own.loss_cold] {
            v.push(r.get::<ratio>().to_string());
        }
        s.push_str(&v.join(","));
        s.push('\n');
    }
    s
}

/// A Markdown table of the per-point disagreement, in percent (1-D vs CFD
/// unless stated).
#[must_use]
pub fn markdown_summary(c: &[Comparison]) -> String {
    let mut s = String::from("| id | split | Re_h | Re_c | k_w | L | NTU_cfd | (a) UA | (a) Q | (a) ε | (a) Nu_h | (a) Nu_c | (a) Δp_h | (a) Δp_c | CFD ε/ε-NTU | (a) ε/ε-NTU | (b) UA | (b) Q | (b) Δp_h | (b) Δp_c | (b) Nu_h used/own | (b) Nu_c used/own |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n");
    let p = |v: f64| format!("{:+.1}", 100.0 * v);
    for k in c {
        let (y, a, b) = (&k.row.output, &k.uncalibrated, &k.with_closures);
        let x = &k.row.input;
        let [rh, rc, kw, l] = x.to_si();
        let (ch, cc) = x.capacity_rates();
        let ntu = y.ua.get::<watt_per_kelvin>() / ch.min(cc);
        let ua = |o: &DoublePipeOutput| o.ua.get::<watt_per_kelvin>();
        let q = |o: &DoublePipeOutput| o.q.get::<watt>();
        let e = |o: &DoublePipeOutput| o.effectiveness.get::<ratio>();
        let dh = |o: &DoublePipeOutput| o.dp_hot.get::<pascal>();
        let dc = |o: &DoublePipeOutput| o.dp_cold.get::<pascal>();
        let nh = |o: &DoublePipeOutput| o.nu_hot.get::<ratio>();
        let nc = |o: &DoublePipeOutput| o.nu_cold.get::<ratio>();
        let _ = writeln!(
            s,
            "| {} | {} | {:.3} | {:.3} | {} | {} | {:.2} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            k.row.id,
            k.row.split.as_str(),
            rh,
            rc,
            kw,
            l,
            ntu,
            p(rel(ua(a), ua(y))),
            p(rel(q(a), q(y))),
            p(rel(e(a), e(y))),
            p(rel(nh(a), nh(y))),
            p(rel(nc(a), nc(y))),
            p(rel(dh(a), dh(y))),
            p(rel(dc(a), dc(y))),
            p(rel(e(y), eps_ntu(x, y.ua))),
            p(rel(e(a), eps_ntu(x, a.ua))),
            p(rel(ua(b), ua(y))),
            p(rel(q(b), q(y))),
            p(rel(dh(b), dh(y))),
            p(rel(dc(b), dc(y))),
            p(rel(k.closures_used.nu_hot.get::<ratio>(), k.closures_own.nu_hot.get::<ratio>())),
            p(rel(k.closures_used.nu_cold.get::<ratio>(), k.closures_own.nu_cold.get::<ratio>())),
        );
    }
    s
}
