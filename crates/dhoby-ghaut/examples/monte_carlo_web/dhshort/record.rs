//! The recorded eigenvalues of the DH shortcuts on the FHR unit cell, **quoted
//! as recorded**, with date, statistics, host and source. Nothing here is
//! computed. Every record predates a change that could move it (the
//! URR/DBRC defaults of 2026-09-20, or the SCLS wiring fix of 2026-09-14), so
//! the whole table is **re-measurement pending (gh:#582)**, as the lesson says.
//!
//! Sources, read 2026-10-07:
//! - 2026-09-14: the doc comment of `outram-mc-libs/examples/dh_keff_vv.rs`
//!   ("Results — measured 2026-09-14") and the lesson's step 5 table.
//! - 2026-09-18: `outram-mc-libs/docs/cls-scls-vv.md`, "Eigenvalue result"
//!   (all seven arms, one run).
//! - 2026-10-02: `outram-mc-libs/verification_and_validation/ring_rpt/ring_rpt_vs_openmc.md`.
//! - 2026-10-05: gh:#582 (delta and CLS only), and the lesson's step 5.

/// One arm of one record: index into [`super::model::TREATMENTS`], `k`, its
/// σ, and the speed against the record's own delta arm (`None`: not recorded).
#[derive(Clone, Copy, Debug)]
pub struct Arm {
    pub t: usize,
    pub k: f64,
    pub sigma: f64,
    pub speed: Option<f64>,
}

/// One recorded run.
#[derive(Clone, Copy, Debug)]
pub struct Record {
    pub date: &'static str,
    pub stats: &'static str,
    pub host: &'static str,
    pub source: &'static str,
    /// Why it is not the current number.
    pub status: &'static str,
    pub arms: &'static [Arm],
}

const fn a(t: usize, k: f64, sigma: f64, speed: Option<f64>) -> Arm {
    Arm { t, k, sigma, speed }
}

pub const RECORDS: [Record; 4] = [
    Record {
        date: "2026-09-14",
        stats: "7200 × [15 + 40], one thread",
        host: "Intel Xeon 2.10 GHz, 4 vCPU (KVM), 1 core used",
        source: "dh_keff_vv.rs doc comment; lesson step 5",
        status:
            "before the URR/DBRC defaults (2026-09-20); the SCLS row may predate its wiring fix",
        arms: &[
            a(0, 1.38647, 0.00211, Some(1.00)),
            a(1, 1.35380, 0.00267, Some(2.16)),
            a(2, 1.35282, 0.00228, Some(2.09)),
            a(3, 1.34345, 0.00234, Some(1.75)),
            a(4, 1.38684, 0.00252, Some(2.13)),
        ],
    },
    Record {
        date: "2026-09-18",
        stats: "800 × [15 + 40], one thread",
        host: "hardware not recorded",
        source: "docs/cls-scls-vv.md, all seven arms in one run",
        status:
            "before the URR/DBRC defaults (2026-09-20); low statistics (σ about 700 pcm per arm)",
        arms: &[
            a(0, 1.38050, 0.00791, Some(1.00)),
            a(1, 1.34901, 0.00727, Some(148.4 / 68.9)),
            a(2, 1.33799, 0.00638, Some(148.4 / 2158.7)),
            a(3, 1.34647, 0.00731, Some(148.4 / 83.1)),
            a(4, 1.39754, 0.00734, Some(148.4 / 68.8)),
            a(5, 1.38719, 0.00662, Some(148.4 / 157.6)),
            a(6, 1.37587, 0.00625, Some(148.4 / 3298.3)),
        ],
    },
    Record {
        date: "2026-10-02",
        stats: "7200 × [15 + 40], one process per arm",
        host: "Intel i9-13900K",
        source: "verification_and_validation/ring_rpt/ring_rpt_vs_openmc.md",
        status: "three arms only; partial re-measurement",
        arms: &[
            a(0, 1.38155, 0.00222, Some(1.00)),
            a(3, 1.34019, 0.00280, Some(1.52)),
            a(4, 1.38394, 0.00204, Some(1.67)),
        ],
    },
    Record {
        date: "2026-10-05",
        stats: "7200 × [15 + 40], one core per arm",
        host: "Intel Xeon 2.10 GHz, 4 vCPU (KVM)",
        source: "gh:#582 (stopped at the hand-over); lesson step 5",
        status: "two arms only; SCLS and ring-RPT not re-run",
        arms: &[
            a(0, 1.38155, 0.00222, Some(1.00)),
            a(1, 1.34536, 0.00247, Some(126.0 / 70.4)),
        ],
    },
];

/// Treatment `t`'s bias against the same record's delta arm: (pcm, combined
/// σ in pcm), or `None` if the record has no such arm (or is the delta arm).
pub fn bias(r: &Record, t: usize) -> Option<(f64, f64)> {
    let d = r.arms.iter().find(|x| x.t == 0)?;
    let x = r.arms.iter().find(|x| x.t == t && t != 0)?;
    Some((
        (x.k - d.k) * 1.0e5,
        (x.sigma.powi(2) + d.sigma.powi(2)).sqrt() * 1.0e5,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The biases the lesson quotes come out of the table as quoted: the
    /// 2026-09-14 row's −3267 / −3365 / −4302 / +37 pcm, 2026-10-02's −4136
    /// and +239, 2026-10-05's −3619 ± 332.
    #[test]
    fn the_table_reproduces_the_quoted_biases() {
        let pcm = |r: usize, t: usize| bias(&RECORDS[r], t).map(|b| b.0.round() as i64);
        assert_eq!(
            [pcm(0, 1), pcm(0, 2), pcm(0, 3), pcm(0, 4)],
            [Some(-3267), Some(-3365), Some(-4302), Some(37)]
        );
        assert_eq!([pcm(2, 3), pcm(2, 4)], [Some(-4136), Some(239)]);
        let (b, s) = bias(&RECORDS[3], 1).unwrap();
        assert_eq!((b.round() as i64, s.round() as i64), (-3619, 332));
        assert_eq!(pcm(1, 5), Some(669));
        assert_eq!(pcm(1, 6), Some(-463));
        assert!(bias(&RECORDS[0], 0).is_none() && bias(&RECORDS[2], 1).is_none());
    }
}
