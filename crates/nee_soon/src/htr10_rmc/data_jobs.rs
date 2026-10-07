// SPDX-License-Identifier: GPL-3.0-only
//! **The HTR-10 nuclide set, built in jobs that can run in different
//! processes** (gh:#786).
//!
//! [`super::data::load_htr10_nuclides`] builds the 38 slots one after the
//! other, in one thread, and reconstructs a tape once per slot (C-12 three
//! times: free, graphite-bound and SiC-bound). The browser demo of the full
//! core spreads the work over a pool of Web Workers that share nothing but
//! messages, so it is cut into:
//!
//! 1. **jobs** ([`Htr10NuclideLayout::processing_jobs`]): each distinct
//!    evaluated tape once (RECONR + BROADR + PURR,
//!    `Nuclide::process_evaluation`) and each distinct thermal law once
//!    (THERMR from its tape, or LEAPR for the UO₂ laws). [`process_job`] runs
//!    one; its [`JobProduct`] encodes to `f64`s exactly
//!    ([`JobProduct::to_f64s`]) to be shipped;
//! 2. **assembly** ([`assemble_slots`]): every worker builds every slot from
//!    the products and its own copy of each tape (`Nuclide::from_processed`,
//!    the cheap half), binding the laws as the loader does.
//!
//! **The nuclides are the loader's, bit for bit**: `Nuclide::from_tape` is
//! `process_evaluation` then `from_processed` (pinned in `outram-mc-libs` by
//! `tests/processed_evaluation_round_trip.rs`), and reconstructing a tape
//! twice gives the same numbers twice. `the_jobs_path_builds_the_loaders_nuclides`
//! checks it on a cut-down layout; the whole layout is the opt-in
//! `the_jobs_path_builds_the_whole_core_set`.
//!
//! Orchestration only: no physics is implemented here.

use njoy_outram_park_fork::endf::tape::Tape as EndfTape;
use njoy_outram_park_fork::leapr::decks::SabMaterial;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::processed::ProcessedEvaluation;
use outram_mc_libs::material::thermal::ThermalScattering;
use uom::si::thermodynamic_temperature::kelvin;

use super::data::{Htr10DataConfig, Htr10DataError, Htr10NuclideLayout, Tape, ThermalLaw, Uo2Laws};

/// RECONR and BROADR tolerance of every slot, as the loader (NJOY's 1e-3).
pub const TOLERANCE: f64 = 1.0e-3;

/// One unit of the expensive processing.
#[derive(Debug, Clone, PartialEq)]
pub enum ProcessingJob {
    /// One evaluated tape: RECONR, BROADR and PURR, shared by every slot
    /// that reads it. `name` is the first such slot's.
    Tape {
        /// The tape.
        tape: Tape,
        /// Nuclide name (`"C12"`).
        name: &'static str,
    },
    /// One thermal-scattering law.
    Law(ThermalLaw),
}

impl ProcessingJob {
    /// What the job is, for progress lines (`"U235"`, `"graphite S(a,b)"`).
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Tape { name, .. } => (*name).to_string(),
            Self::Law(l) => l.label().to_string(),
        }
    }

    /// The tape the job reads, or `None` for a law LEAPR generates.
    ///
    /// # Errors
    ///
    /// [`Htr10DataError::InvalidConfig`] for UO₂ laws read from a folder of
    /// tapes (`Uo2Laws::Tapes`), which this path does not carry.
    pub fn tape(&self, cfg: &Htr10DataConfig) -> Result<Option<Tape>, Htr10DataError> {
        Ok(match self {
            Self::Tape { tape, .. } => Some(*tape),
            Self::Law(ThermalLaw::Graphite { file, .. }) => Some(Tape::endf(file)),
            Self::Law(ThermalLaw::CInSiC) => Some(Tape::endf("tsl-CinSiC.endf")),
            Self::Law(ThermalLaw::SiInSiC) => Some(Tape::endf("tsl-SiinSiC.endf")),
            Self::Law(ThermalLaw::UInUO2 | ThermalLaw::OInUO2) => match cfg.uo2_laws {
                Uo2Laws::GeneratedFromLeapr => None,
                Uo2Laws::Tapes(_) => {
                    return Err(Htr10DataError::InvalidConfig(
                        "the shared-job path builds the UO2 laws with LEAPR only".into(),
                    ))
                }
            },
        })
    }
}

/// What a job produces.
#[derive(Debug, Clone)]
pub enum JobProduct {
    /// A tape's RECONR + BROADR + PURR products.
    Nuclide(ProcessedEvaluation),
    /// A thermal law.
    Law(ThermalScattering),
}

impl JobProduct {
    /// `[0, ProcessedEvaluation words...]` or `[1, ThermalScattering words...]`.
    #[must_use]
    pub fn to_f64s(&self) -> Vec<f64> {
        let (kind, words) = match self {
            Self::Nuclide(p) => (0.0, p.to_f64s()),
            Self::Law(l) => (1.0, l.to_f64s()),
        };
        let mut v = Vec::with_capacity(words.len() + 1);
        v.push(kind);
        v.extend(words);
        v
    }

    /// The inverse of [`Self::to_f64s`].
    ///
    /// # Errors
    ///
    /// [`Htr10DataError::InvalidConfig`] for words that are not a product.
    pub fn from_f64s(v: &[f64]) -> Result<Self, Htr10DataError> {
        let bad = |e: njoy_outram_park_fork::NjoyError| {
            Htr10DataError::InvalidConfig(format!("job product: {e}"))
        };
        match v.first() {
            Some(k) if *k == 0.0 => Ok(Self::Nuclide(
                ProcessedEvaluation::from_f64s(&v[1..]).map_err(bad)?,
            )),
            Some(k) if *k == 1.0 => Ok(Self::Law(
                ThermalScattering::from_f64s(&v[1..]).map_err(bad)?,
            )),
            _ => Err(Htr10DataError::InvalidConfig(
                "job product: unknown kind".into(),
            )),
        }
    }
}

impl Htr10NuclideLayout {
    /// Every distinct thermal law (in first-use order, as the loader takes
    /// them), then every distinct tape (in slot order).
    #[must_use]
    pub fn processing_jobs(&self) -> Vec<ProcessingJob> {
        let mut jobs: Vec<ProcessingJob> = Vec::new();
        for law in self.slots.iter().filter_map(|s| s.thermal) {
            if !jobs.contains(&ProcessingJob::Law(law)) {
                jobs.push(ProcessingJob::Law(law));
            }
        }
        for s in &self.slots {
            if !jobs
                .iter()
                .any(|j| matches!(j, ProcessingJob::Tape { tape, .. } if *tape == s.tape))
            {
                jobs.push(ProcessingJob::Tape {
                    tape: s.tape,
                    name: s.name,
                });
            }
        }
        jobs
    }
}

fn mat_of(t: &EndfTape, what: &str) -> Result<i32, Htr10DataError> {
    t.materials()
        .first()
        .copied()
        .ok_or_else(|| Htr10DataError::InvalidConfig(format!("{what}: no material on the tape")))
}

/// Run one job. `tape` is the tape [`ProcessingJob::tape`] names, already
/// read (`None` for a LEAPR law).
///
/// # Errors
///
/// [`Htr10DataError::InvalidConfig`] when the tape is missing or fails to
/// process, with the reason.
pub fn process_job(
    job: &ProcessingJob,
    cfg: &Htr10DataConfig,
    tape: Option<&EndfTape>,
) -> Result<JobProduct, Htr10DataError> {
    let t_k = cfg.temperature.get::<kelvin>();
    let what = job.label();
    let fail =
        |e: njoy_outram_park_fork::NjoyError| Htr10DataError::InvalidConfig(format!("{what}: {e}"));
    let need = || {
        tape.ok_or_else(|| {
            Htr10DataError::InvalidConfig(format!("{what}: its tape was not supplied"))
        })
    };
    Ok(match job {
        ProcessingJob::Tape { .. } => {
            let t = need()?;
            JobProduct::Nuclide(
                Nuclide::process_evaluation(t, mat_of(t, &what)?, t_k, TOLERANCE, TOLERANCE)
                    .map_err(fail)?,
            )
        }
        // The loader's labels: `c_Graphite`, `c_SiC`, `Si_SiC`, `U_UO2`, `O_UO2`.
        ProcessingJob::Law(law) => JobProduct::Law(match law {
            ThermalLaw::Graphite { mat, .. } => {
                ThermalScattering::from_tape(need()?, *mat, t_k, "c_Graphite").map_err(fail)?
            }
            ThermalLaw::CInSiC => {
                ThermalScattering::from_tape(need()?, 44, t_k, "c_SiC").map_err(fail)?
            }
            ThermalLaw::SiInSiC => {
                ThermalScattering::from_tape(need()?, 43, t_k, "Si_SiC").map_err(fail)?
            }
            ThermalLaw::UInUO2 => {
                ThermalScattering::from_leapr(SabMaterial::UInUO2, t_k, "U_UO2").map_err(fail)?
            }
            ThermalLaw::OInUO2 => {
                ThermalScattering::from_leapr(SabMaterial::OInUO2, t_k, "O_UO2").map_err(fail)?
            }
        }),
    })
}

/// Build every slot of `layout` from the jobs' products, in slot order.
/// `product(i)` supplies the product of `jobs[i]` (as
/// [`Htr10NuclideLayout::processing_jobs`] lists them) when it is needed,
/// once each: the laws first, then one tape's product at a time, so only one
/// processed tape is decoded at once. `read_tape` supplies each tape; each is
/// read once, used for every slot that reads it, and dropped.
///
/// # Errors
///
/// A product missing or of the wrong kind, a tape that fails to read, or a
/// nuclide that fails to assemble.
pub fn assemble_slots(
    cfg: &Htr10DataConfig,
    layout: &Htr10NuclideLayout,
    jobs: &[ProcessingJob],
    mut product: impl FnMut(usize) -> Result<JobProduct, Htr10DataError>,
    mut read_tape: impl FnMut(&Tape) -> Result<EndfTape, Htr10DataError>,
) -> Result<Vec<Nuclide>, Htr10DataError> {
    let t_k = cfg.temperature.get::<kelvin>();
    let mut laws: Vec<(ThermalLaw, ThermalScattering)> = Vec::new();
    for (i, job) in jobs.iter().enumerate() {
        if let ProcessingJob::Law(l) = job {
            match product(i)? {
                JobProduct::Law(t) => laws.push((*l, t)),
                JobProduct::Nuclide(_) => {
                    return Err(Htr10DataError::InvalidConfig(format!(
                        "{}: not a law",
                        l.label()
                    )))
                }
            }
        }
    }
    let mut built: Vec<Option<Nuclide>> = (0..layout.slots.len()).map(|_| None).collect();
    for (i, job) in jobs.iter().enumerate() {
        let ProcessingJob::Tape { tape, .. } = job else {
            continue;
        };
        let users: Vec<usize> = (0..layout.slots.len())
            .filter(|&k| layout.slots[k].tape == *tape)
            .collect();
        if users.is_empty() {
            continue;
        }
        let JobProduct::Nuclide(processed) = product(i)? else {
            return Err(Htr10DataError::InvalidConfig(format!(
                "{}: not a processed tape",
                tape.file
            )));
        };
        let t = read_tape(tape)?;
        let mat = mat_of(&t, tape.file)?;
        for k in users {
            let s = &layout.slots[k];
            let n = Nuclide::from_processed(&t, mat, s.name, t_k, processed.clone())
                .map_err(|e| Htr10DataError::InvalidConfig(format!("{}: {e}", s.name)))?;
            built[k] = Some(match s.thermal {
                Some(l) => {
                    let law = laws.iter().find(|(m, _)| *m == l).ok_or_else(|| {
                        Htr10DataError::InvalidConfig(format!("no product for {}", l.label()))
                    })?;
                    n.with_thermal_scattering(law.1.clone())
                }
                None => n,
            });
        }
    }
    built
        .into_iter()
        .enumerate()
        .map(|(k, n)| {
            n.ok_or_else(|| {
                Htr10DataError::InvalidConfig(format!(
                    "slot {k} ({}) has no job",
                    layout.slots[k].name
                ))
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::htr10_rmc::data::{load_htr10_nuclides, DataDir};
    use outram_mc_libs::run_diagnostics::RunDiagnostics;

    fn read(t: &Tape) -> Result<EndfTape, Htr10DataError> {
        EndfTape::read_file(&t.path())
            .map_err(|e| Htr10DataError::InvalidConfig(format!("{}: {e}", t.file)))
    }

    /// The default layout's jobs: every law once, every tape once, every
    /// slot's tape and law among them; 5 laws and 31 tapes for 38 slots (43 items in the loader's log).
    #[test]
    fn every_slot_has_exactly_one_job_for_its_tape_and_law() {
        let layout = Htr10NuclideLayout::plan(&Htr10DataConfig::default()).expect("plan");
        let jobs = layout.processing_jobs();
        let laws = jobs
            .iter()
            .filter(|j| matches!(j, ProcessingJob::Law(_)))
            .count();
        assert_eq!((laws, jobs.len() - laws, layout.len()), (5, 31, 38));
        for s in &layout.slots {
            assert_eq!(
                jobs.iter()
                    .filter(|j| matches!(j, ProcessingJob::Tape { tape, .. } if *tape == s.tape))
                    .count(),
                1,
                "{}",
                s.name
            );
            if let Some(l) = s.thermal {
                assert!(jobs.contains(&ProcessingJob::Law(l)));
            }
        }
        // Laws first: the loader processes them before any nuclide.
        let first_tape = jobs
            .iter()
            .position(|j| matches!(j, ProcessingJob::Tape { .. }))
            .expect("tapes");
        assert!(jobs[..first_tape]
            .iter()
            .all(|j| matches!(j, ProcessingJob::Law(_))));
        let cfg = Htr10DataConfig::default();
        assert!(
            jobs.iter()
                .filter(|j| j.tape(&cfg).expect("tape").is_none())
                .count()
                == 2,
            "two LEAPR laws"
        );
        assert!(jobs
            .iter()
            .any(|j| matches!(j.tape(&cfg), Ok(Some(t)) if t.dir == DataDir::AceSubmoduleEndfB8)));
    }

    /// The jobs path and the loader give the same nuclides, bit for bit, on
    /// a cut-down layout (helium, boron, the SiC silicon with its bound law):
    /// cross sections at 600 energies and thermal samples, after every
    /// product has crossed as `f64`s.
    #[test]
    fn the_jobs_path_builds_the_loaders_nuclides() {
        let cfg = Htr10DataConfig::default();
        let mut layout = Htr10NuclideLayout::plan(&cfg).expect("plan");
        layout.slots.retain(|s| {
            ["He3", "He4", "B10", "B11", "Si29"].contains(&s.name)
                && s.thermal.is_none_or(|l| l == ThermalLaw::SiInSiC)
        });
        layout.slots.dedup_by(|a, b| a.name == b.name);
        if layout.slots.iter().any(|s| !s.tape.path().exists()) {
            eprintln!("SKIP: tapes not in this checkout");
            return;
        }
        let jobs = layout.processing_jobs();
        assert_eq!(
            jobs.iter()
                .filter(|j| matches!(j, ProcessingJob::Law(_)))
                .count(),
            1
        );
        let products: Vec<JobProduct> = jobs
            .iter()
            .map(|j| {
                let t = j.tape(&cfg).expect("tape").map(|t| read(&t).expect("read"));
                let p = process_job(j, &cfg, t.as_ref()).expect("process");
                JobProduct::from_f64s(&p.to_f64s()).expect("decode")
            })
            .collect();
        let shared = assemble_slots(&cfg, &layout, &jobs, |i| Ok(products[i].clone()), read)
            .expect("assemble");
        let direct =
            load_htr10_nuclides(&cfg, &layout, &mut RunDiagnostics::new("test")).expect("load");
        assert_eq!(shared.len(), direct.len());
        for (i, (a, b)) in shared.iter().zip(&direct).enumerate() {
            for k in 0..600 {
                let e = 1.0e-5 * (2.0e7_f64 / 1.0e-5).powf(f64::from(k) / 599.0);
                let (x, y) = (a.xs_at_energy(e, 300.15), b.xs_at_energy(e, 300.15));
                assert_eq!(format!("{x:?}"), format!("{y:?}"), "slot {i} at {e}");
                let (mut s1, mut s2) = (k as u64, k as u64);
                assert_eq!(
                    format!("{:?}", a.sample_thermal(e, &mut s1)),
                    format!("{:?}", b.sample_thermal(e, &mut s2))
                );
            }
        }
        assert!(
            shared
                .iter()
                .any(|n| n.sample_thermal(0.0253, &mut 1u64).is_some()),
            "the SiC silicon carries its law"
        );
        assert!(JobProduct::from_f64s(&[9.0]).is_err());
        assert!(assemble_slots(
            &cfg,
            &layout,
            &jobs,
            |i| Ok(products[(i + 1) % products.len()].clone()),
            read
        )
        .is_err());
    }

    /// The whole default layout (38 slots, 36 jobs, minutes): the jobs path
    /// and `load_htr10_nuclides` agree bit for bit at 300 energies per slot.
    #[test]
    #[ignore = "processes every HTR-10 tape twice (minutes); run with --ignored"]
    fn the_jobs_path_builds_the_whole_core_set() {
        let cfg = Htr10DataConfig::default();
        let layout = Htr10NuclideLayout::plan(&cfg).expect("plan");
        let jobs = layout.processing_jobs();
        let products: Vec<JobProduct> = jobs
            .iter()
            .map(|j| {
                let t = j.tape(&cfg).expect("tape").map(|t| read(&t).expect("read"));
                process_job(j, &cfg, t.as_ref()).expect("process")
            })
            .collect();
        let shared = assemble_slots(&cfg, &layout, &jobs, |i| Ok(products[i].clone()), read)
            .expect("assemble");
        let direct =
            load_htr10_nuclides(&cfg, &layout, &mut RunDiagnostics::new("test")).expect("load");
        for (i, (a, b)) in shared.iter().zip(&direct).enumerate() {
            for k in 0..300 {
                let e = 1.0e-5 * (2.0e7_f64 / 1.0e-5).powf(f64::from(k) / 299.0);
                assert_eq!(
                    format!("{:?}", a.xs_at_energy(e, 300.15)),
                    format!("{:?}", b.xs_at_energy(e, 300.15)),
                    "slot {i} at {e}"
                );
            }
        }
    }
}
