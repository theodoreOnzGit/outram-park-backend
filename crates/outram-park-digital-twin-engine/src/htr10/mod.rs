//! # HTR-10 cited design data and packed-bed correlations
//!
//! Foundation layer for the HTR-10 pebble-bed simulator rewrite (bead
//! `op-jyyp`): the published design constants of the HTR-10 test reactor and
//! the packed-bed closure correlations its core model needs, each carrying its
//! literature citation, together with unit tests that reproduce published
//! numbers. This module is the start of the simulator's V&V record.
//!
//! ## What belongs here / what does not
//!
//! - **Belongs here:** *cited* HTR-10 design constants ([`design`]), the KTA
//!   packed-bed pressure-drop correlation ([`kta`]), the tabulated
//!   Zehner-Bauer-Schlunder effective bed conductivity ([`zbs`]), the IAEA
//!   core-physics benchmark specification and published reference eigenvalues
//!   ([`neutronics`]), and the unit tests that check them against the
//!   published values they came from.
//! - **Does NOT belong here:** the simulator itself (solver loops, transient
//!   models, GUI) — that is the `htgr_sim_v1` rewrite tracked under bead
//!   `op-jyyp` — and any number that cannot cite a published source. Do not
//!   add uncited "reasonable" constants to this module.
//!
//! This module is a deliberate, maintainer-directed exception (bead `op-jyyp`,
//! 2026-08-11) to this crate's "no new physics in the library" rule: cited
//! constants and their reference correlations live here so that the example
//! rewrite and its V&V tests share one provenance-checked source.
//!
//! ## Sources
//!
//! | Source | Access tier | On-disk catalogue |
//! |---|---|---|
//! | IAEA-TECDOC-1382, *Evaluation of high temperature gas cooled reactor performance: Benchmark analysis related to initial testing of the HTTR and HTR-10*, IAEA Vienna, November 2003 — Chapter 4 is the HTR-10 core physics benchmark | Open | `crates/kovan-literature/open/reports/iaea-tecdoc-1382-part2.json` (Chapter 4; `part1` is the HTTR half and front matter). |
//! | Gao & Shi (2002), Nucl. Eng. Des. 218, 51-64, doi 10.1016/S0029-5493(02)00198-X | Proprietary (cited, not re-hosted) | `crates/kovan-literature/proprietary/papers/gao2002htr10th.json` (kovan-ddb61cb136fb98a9) |
//! | Virtual Test Bed generic pebble-bed tutorial, step 2 (KTA worked example) | Open | `reference-data/virtual_test_bed/doc/content/htgr/generic-pbr-tutorial/step2.md` |
//! | Virtual Test Bed generic PBR input (ZBS conductivity tabulation) | Open | `reference-data/virtual_test_bed/htgr/generic-pbr/pbr.i` |
//!
//! ## Status: NOT VALIDATED
//!
//! Nothing in this module validates an HTR-10 *simulator*. The tests here
//! establish only that the constants are transcribed correctly, are
//! self-consistent, and that the correlations reproduce published worked
//! examples. Per `RESPONSIBLE_USE.md` this is untrusted draft material until
//! human-reviewed; the full V&V sequence (PBMR-400 coupled benchmark, HTR-10
//! criticality, safety demonstration tests) is bead `op-jyyp.11`.
//!
//! ## Android / portability
//!
//! This module is GUI-free and, like [`crate::animation`], builds on
//! Android/Termux. Do not add `egui`/`eframe` imports here.

pub mod design;
pub mod kta;
pub mod neutronics;
pub mod zbs;
