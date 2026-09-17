//! Bibliography for the pebble-bed / dispersion-fuel geometry methods.
//!
//! These are the stochastic-media and packing-generation papers the
//! [`super::sphere_packing`] and [`super::delta_tracking`] work builds on —
//! Zhe Chuan Tan et al.'s dispersion-fuel series in the RMC Monte Carlo code.
//! Each is a machine-readable [`Reference`] so doc comments elsewhere can point at
//! a concrete, rust-analyzer-navigable citation rather than a bare DOI string.
//!
//! Cite them from a doc comment like:
//! ```
//! use outram_mc_libs::pebble_beds::references::TAN2024_RSA;
//! assert_eq!(TAN2024_RSA.year, 2024);
//! ```

/// One journal-article citation, in the fields a reader (or a `.bib` exporter)
/// needs. All strings are `'static` — these are compile-time constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reference {
    /// Author list, "Last, First and Last, First …" (BibTeX `author` order).
    pub authors: &'static str,
    /// Article title.
    pub title: &'static str,
    /// Journal name.
    pub journal: &'static str,
    /// Publication year.
    pub year: u16,
    /// Digital Object Identifier (without the `https://doi.org/` prefix), or `""`
    /// when the publisher has not assigned/exposed one.
    pub doi: &'static str,
}

impl Reference {
    /// Render this reference as a BibTeX `@article` entry (for exporting a `.bib`).
    ///
    /// The citation key is `<firstauthorlastname><year>` l-cased — matching the
    /// keys used in the source manuscripts (e.g. `tan2024`).
    pub fn to_bibtex(&self) -> String {
        let key = format!("tan{}", self.year);
        let doi_line = if self.doi.is_empty() {
            String::new()
        } else {
            format!("  doi       = {{{}}},\n", self.doi)
        };
        format!(
            "@article{{{key},\n  \
             author    = {{{}}},\n  \
             title     = {{{}}},\n  \
             journal   = {{{}}},\n  \
             year      = {{{}}},\n{}}}",
            self.authors, self.title, self.journal, self.year, doi_line
        )
    }
}

/// Tan, Feng, Wang (2024) — improved parallel Random Sequential Addition (RSA) for
/// generating dispersion-fuel (TRISO) packings in RMC. The packing generator whose
/// output the doubly-heterogeneous transport in this module consumes.
pub const TAN2024_RSA: Reference = Reference {
    authors: "Tan, Zhe Chuan and Feng, Zhi Yuan and Wang, Kan",
    title: "An improved parallel Random Sequential Addition algorithm in RMC code for dispersion fuel analysis",
    journal: "Annals of Nuclear Energy",
    year: 2024,
    doi: "10.1016/j.anucene.2024.110439",
};

/// Tan, Feng, Chan, Wang (2025) — a semi-implicit Chord Length Sampling (CLS)
/// method for dispersion fuel. CLS is the on-the-fly stochastic-geometry sampling
/// alternative to an explicit packing; relevant to [`super::delta_tracking`].
pub const TAN2025_CLS: Reference = Reference {
    authors: "Tan, Zhe Chuan and Feng, Zhi Yuan and Chan, Kok Yue and Wang, Kan",
    title: "A semi-implicit Chord Length Sampling method for dispersion fuel analysis",
    journal: "Annals of Nuclear Energy",
    year: 2025,
    doi: "10.1016/j.anucene.2025.111436",
};

/// Tan, Feng, Wang (2026) — an iterative RSA–DEM (Discrete Element Method) method
/// for reaching *high* particle packing fractions in stochastic media, beyond what
/// plain RSA saturates at.
pub const TAN2026_RSA_DEM: Reference = Reference {
    authors: "Tan, Zhe Chuan and Feng, Zhi Yuan and Wang, Kan",
    title: "An Iterative RSA-DEM Method for High Particle Packing Fraction Stochastic Media",
    journal: "Nuclear Science and Engineering",
    year: 2026,
    doi: "10.1080/00295639.2025.2456456",
};

/// Tan, Feng, Wang (2026) — coupled ODR–DEM (Ordered/Overlap-Driven Relaxation with
/// DEM) packing methods for dispersion-fuel analysis in RMC.
pub const TAN2026_ODR_DEM: Reference = Reference {
    authors: "Tan, Zhe Chuan and Feng, Zhi Yuan and Wang, Kan",
    title: "Coupled ODR-DEM methods for dispersion fuel analysis in RMC code",
    journal: "Journal of Nuclear Science and Technology",
    year: 2026,
    doi: "",
};

/// Every reference in this bibliography, for iterating or exporting a full `.bib`.
pub const ALL: [Reference; 4] = [TAN2024_RSA, TAN2025_CLS, TAN2026_RSA_DEM, TAN2026_ODR_DEM];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bibtex_round_trips_key_fields() {
        let b = TAN2024_RSA.to_bibtex();
        assert!(b.starts_with("@article{tan2024,"));
        assert!(b.contains("Annals of Nuclear Energy"));
        assert!(b.contains("10.1016/j.anucene.2024.110439"));
    }

    #[test]
    fn missing_doi_is_omitted() {
        // The JNST entry has no DOI recorded; the field must simply not appear.
        assert!(!TAN2026_ODR_DEM.to_bibtex().contains("doi"));
    }

    #[test]
    fn bibliography_is_complete() {
        assert_eq!(ALL.len(), 4);
        assert!(ALL.iter().all(|r| r.authors.starts_with("Tan, Zhe Chuan")));
    }
}
