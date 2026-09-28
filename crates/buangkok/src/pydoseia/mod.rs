// SPDX-License-Identifier: GPL-3.0-only
//! # A port of pyDOSEIA: Gaussian-plume dilution and five dose pathways
//!
//! > **Research, education and V&V only.** Not for medical, occupational,
//! > public-health, emergency-response, licensing or regulatory use, and
//! > never a dose to a real person or population (`RESPONSIBLE_USE.md`).
//!
//! ## Upstream and provenance
//!
//! | | |
//! |---|---|
//! | Upstream project | **pyDOSEIA**, <https://github.com/BiswajitSadhu/pyDOSEIA> |
//! | Commit ported | `dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce` (branch `head`, 2025-08-26), checked 2026-09-28 |
//! | Source files | `metfunc.py`, `dosefunc.py`, `raddcffunc.py`, `outputfunc.py`, `main.py`, the input generator's defaults (function-level mapping in each submodule) |
//! | Copyright | Copyright (c) 2024 Dr. Biswajit Sadhu |
//! | Licence | MIT. The full notice is in `crates/buangkok/NOTICE` and must stay with every ported file. MIT is compatible with this crate's GPL-3.0 |
//! | Paper | B. Sadhu, T. Sarkar, S. Anand, K. D. Singh, D. K. Aswal, "pyDOSEIA: A Python Package for Radiological Impact Assessment during Long-term or Accidental Atmospheric Releases", *Health Physics* **130**(1) (2026) 94-110, doi:[10.1097/HP.0000000000002014](https://doi.org/10.1097/HP.0000000000002014), PMID [40622262](https://pubmed.ncbi.nlm.nih.gov/40622262/). The article is (c) 2025 Health Physics Society: cited, not reproduced |
//!
//! ## What is ported, and how it is verified
//!
//! **All of pyDOSEIA's computation** (2026-09-28, two tranches); what is not
//! ported is I/O and UI (Excel reading, plots, text formatting, the YAML
//! dialogue, joblib), listed function by function in
//! `docs/pydoseia-port-scoping.md`.
//!
//! | Module | Upstream | Status |
//! |---|---|---|
//! | [`met`](crate::pydoseia::met) | met processing: gap filling, TJFD, missing and calm corrections, speed distribution | ported, code-to-code verified |
//! | [`dispersion`](crate::pydoseia::dispersion) | sigmas, height correction, master equations, dilution factor | ported, code-to-code verified (3 modes); the 4th (single plume with met data, D3) as a labelled divergence |
//! | [`nuclide`](crate::pydoseia::nuclide) | half-life text parsing, `0.693 / T` | ported, code-to-code verified |
//! | [`dcf`](crate::pydoseia::dcf) | age brackets, absorption-type lookup, progeny correction | ported, code-to-code verified on **synthetic** tables |
//! | [`dose`](crate::pydoseia::dose) | inhalation, ground shine, submersion, deposition velocity, weathering | ported, code-to-code verified |
//! | [`ingestion`](crate::pydoseia::ingestion) | SRS 19 food chain, H-3 and C-14 models | ported, code-to-code verified (faithful driver, D8-D14); corrected per-nuclide driver as a divergence |
//! | [`plume_shine`](crate::pydoseia::plume_shine) | finite-cloud gamma dose, photon tables, integration limits, point source | ported, code-to-code verified, bit-exact |
//! | [`quadpack`](crate::pydoseia::quadpack) | SciPy's QUADPACK `dqagse` and `tplquad` (what plume shine runs on) | ported, verified against SciPy directly |
//! | [`dcf_screening`](crate::pydoseia::dcf_screening) | multi-source DCF screening (report only, D7) | ported, code-to-code verified |
//! | [`plume_rise`](crate::pydoseia::plume_rise) | plume rise, building wake (never called upstream) | ported / labelled divergences (D5, D6) |
//! | [`config`](crate::pydoseia::config) | the run configuration and its defaults | ported (schema and checks) |
//! | [`assessment`](crate::pydoseia::assessment) | the driver and the summary tables | ported, code-to-code verified against upstream's joblib run |
//!
//! Verification is **code-to-code against upstream itself**: the upstream
//! Python is executed over a grid of inputs by
//! `verification_and_validation/pydoseia_code_to_code/gen_pydoseia_reference.py`
//! and the port must reproduce every number
//! (`tests/pydoseia_code_to_code.rs`; methodology and measured results in
//! `docs/pydoseia-code-to-code.md`). This checks the **translation**, not the
//! physics: agreeing with pyDOSEIA says nothing about whether pyDOSEIA's model
//! is right.
//!
//! ## Faithful, including upstream's defects
//!
//! The port reproduces upstream's control flow and constants, including the
//! defects found while porting (D1-D26 in `docs/pydoseia-code-to-code.md`).
//! Where a defect is clear, a corrected variant sits beside the faithful one
//! and is labelled as a **divergence**, e.g.
//! [`dispersion::CalmCorrection::LowestSpeedClassTotal`](crate::pydoseia::dispersion::CalmCorrection::LowestSpeedClassTotal) (D1),
//! [`dispersion::max_dilution_factor`](crate::pydoseia::dispersion::max_dilution_factor) (D2),
//! [`ingestion::corrected`](crate::pydoseia::ingestion::corrected) (D8-D11) and
//! [`assessment::SummaryIngestion::PerNuclideRowsOnly`](crate::pydoseia::assessment::SummaryIngestion::PerNuclideRowsOnly) (D20).
//!
//! ## No data tables
//!
//! No dose coefficient, transfer factor, photon line or attenuation
//! coefficient, half-life, decay chain or met record from upstream's
//! `library/` or `met_data/` is in this crate. The caller supplies them in
//! upstream's column layouts (see [`dcf`](crate::pydoseia::dcf),
//! [`ingestion`](crate::pydoseia::ingestion),
//! [`plume_shine`](crate::pydoseia::plume_shine) and
//! [`dcf_screening`](crate::pydoseia::dcf_screening) for the per-table licence
//! reasoning). A whole run is in `examples/pydoseia_assessment.rs`.
//!
//! ## Minimal example (inhalation, one nuclide, synthetic coefficient)
//!
//! ```
//! use buangkok::pydoseia::dispersion::{self, MeanSpeedScaling, PlumeGeometry, Receptor};
//! use buangkok::pydoseia::dose::{self, Release};
//! use uom::si::f64::{Length, Radioactivity};
//! use uom::si::length::meter;
//! use uom::si::radioactivity::becquerel;
//!
//! let geometry = PlumeGeometry {
//!     release_height: Length::new::<meter>(30.0),
//!     measurement_height: Length::new::<meter>(10.0),
//!     receptor: Receptor::GroundLevelCentreline,
//! };
//! let per_class = dispersion::dilution_long_term_no_met(
//!     Length::new::<meter>(800.0), geometry, MeanSpeedScaling::UnitSpeed);
//! let chi_over_q = dispersion::max_dilution_factor(&per_class);
//! let release = Release::AnnualDischarge(Radioactivity::new::<becquerel>(1.0e9));
//! let synthetic_dcf_sv_per_bq = 1.0e-9; // not a real coefficient
//! let d = dose::inhalation_dose(chi_over_q, release, synthetic_dcf_sv_per_bq, 40.0).unwrap();
//! assert!(d.millisieverts() > 0.0);
//! ```

pub mod assessment;
pub mod config;
pub(crate) mod csv;
pub mod dcf;
pub mod dcf_screening;
pub mod dispersion;
pub mod dose;
pub mod ingestion;
pub mod met;
pub mod nuclide;
pub mod plume_rise;
pub mod plume_shine;
pub mod quadpack;
pub mod units;
