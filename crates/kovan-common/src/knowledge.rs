//! Knowledge-graph provenance types: correlations, benchmarks, validation
//! cases, and generated-code provenance.
//!
//! These express the "Paper → Correlation → Implementation → Validation" chain
//! (`docs/kovan.md`, "Integration Vision") as records linked by the string IDs
//! defined across `kovan-common`, rather than by direct object references.

/// An engineering correlation (e.g. a Nusselt-number correlation) linking a
/// literature source to an implementation and validation evidence.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KovanCorrelation {
    /// Stable identifier.
    pub id: String,
    /// Human-readable name (e.g. `"Dittus-Boelter"`).
    pub name: String,
    /// ID of the [`crate::KovanDocument`] this correlation is sourced from.
    /// `None` if the source has not been catalogued yet.
    pub source_document_id: Option<String>,
}

/// A benchmark specification (e.g. an ICSBEP critical-experiment evaluation).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KovanBenchmark {
    /// Stable identifier.
    pub id: String,
    /// Display name (e.g. `"HEU-MET-FAST-001 (Godiva)"`).
    pub name: String,
    /// ID of the [`crate::KovanDocument`] describing the benchmark, if any.
    /// `None` if the source evaluation has not been catalogued yet.
    pub source_document_id: Option<String>,
}

/// A validation case tying an implementation to a benchmark and its measured
/// result — the provenance record KOVAN ultimately aims to produce.
///
/// The measured result itself (e.g. `k_eff = 1.00042 ± 0.00015`) is
/// intentionally not modelled here yet: the shape of that data depends on
/// what kind of case it is (a k-eigenvalue benchmark vs. a correlation
/// accuracy check vs. a thermal-hydraulic transient comparison have
/// different result schemas), and no consumer of this type exists yet to
/// drive that design. Adding it speculatively would risk guessing wrong;
/// see `DECISIONS.md`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KovanValidationCase {
    /// Stable identifier.
    pub id: String,
    /// Display name (e.g. `"TUAS Dittus-Boelter vs. Godiva Nu correlation"`).
    pub name: String,
    /// ID of the [`KovanBenchmark`] this case is validated against, if any.
    /// `None` if this case validates a [`KovanCorrelation`] directly instead
    /// (not every validation case is benchmark-based).
    pub benchmark_id: Option<String>,
    /// ID of the [`crate::KovanSymbol`] (the code implementing the correlation
    /// or model under test) exercised by this case, if any. `None` if the
    /// implementation has not been linked yet.
    pub implementation_symbol_id: Option<String>,
}

/// Provenance record for a piece of code emitted by `kovan-codegen`.
///
/// This closes the "Correlation → Implementation" link of the KOVAN integration
/// vision: it records *what* numerical method was generated, the generated
/// source, and (optionally) which [`KovanCorrelation`] it implements and which
/// [`crate::KovanDocument`] the method derives from. It carries no behaviour —
/// it is a serialisable audit record so a generated kernel can be traced back to
/// the paper and correlation it came from.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GeneratedArtifact {
    /// The numerical method that was generated, as a stable label. Produced by
    /// `kovan-codegen` from its `Method` enum (e.g. `"Ode(Rk4)"`). Free text
    /// here so `kovan-common` need not depend on the codegen catalogue enums.
    pub method: String,
    /// The generated Rust source, verbatim as `kovan-codegen` emitted it. May
    /// be an empty string when only the provenance link is being recorded (the
    /// source is stored elsewhere).
    pub source: String,
    /// ID of the [`KovanCorrelation`] this generated code implements, if the
    /// generation was tied to one. `None` for a bare method generation.
    pub correlation_id: Option<String>,
    /// ID of the [`crate::KovanDocument`] (the paper/report) the method derives
    /// from, if known. `None` when the source has not been linked.
    pub source_document_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correlation_json_and_toml_round_trip() {
        let correlation = KovanCorrelation {
            id: "corr-1".into(),
            name: "Dittus-Boelter".into(),
            source_document_id: Some("doc-1".into()),
        };
        let json = serde_json::to_string(&correlation).expect("serialise");
        assert_eq!(
            correlation,
            serde_json::from_str(&json).expect("deserialise")
        );
        let toml_str = toml::to_string(&correlation).expect("toml serialise");
        assert_eq!(
            correlation,
            toml::from_str(&toml_str).expect("toml deserialise")
        );
    }

    #[test]
    fn correlation_with_no_source_round_trips() {
        let correlation = KovanCorrelation {
            id: "corr-1".into(),
            name: "Dittus-Boelter".into(),
            source_document_id: None,
        };
        let json = serde_json::to_string(&correlation).expect("serialise");
        assert_eq!(
            correlation,
            serde_json::from_str(&json).expect("deserialise")
        );
        // toml round trip with a leading-None field still works: toml's
        // Option handling omits the key on serialise and serde's built-in
        // "missing optional field is None" rule restores it on deserialise.
        let toml_str = toml::to_string(&correlation).expect("toml serialise");
        assert_eq!(
            correlation,
            toml::from_str(&toml_str).expect("toml deserialise")
        );
    }

    #[test]
    fn benchmark_json_and_toml_round_trip() {
        let benchmark = KovanBenchmark {
            id: "bench-godiva".into(),
            name: "HEU-MET-FAST-001 (Godiva)".into(),
            source_document_id: Some("icsbep-heu-met-fast-001".into()),
        };
        let json = serde_json::to_string(&benchmark).expect("serialise");
        assert_eq!(benchmark, serde_json::from_str(&json).expect("deserialise"));
        let toml_str = toml::to_string(&benchmark).expect("toml serialise");
        assert_eq!(
            benchmark,
            toml::from_str(&toml_str).expect("toml deserialise")
        );
    }

    #[test]
    fn validation_case_json_and_toml_round_trip() {
        let case = KovanValidationCase {
            id: "val-1".into(),
            name: "TUAS Dittus-Boelter vs. Godiva Nu correlation".into(),
            benchmark_id: Some("bench-godiva".into()),
            implementation_symbol_id: Some("sym-1".into()),
        };
        let json = serde_json::to_string(&case).expect("serialise");
        assert_eq!(case, serde_json::from_str(&json).expect("deserialise"));
        let toml_str = toml::to_string(&case).expect("toml serialise");
        assert_eq!(case, toml::from_str(&toml_str).expect("toml deserialise"));
    }

    #[test]
    fn generated_artifact_json_and_toml_round_trip() {
        let artifact = GeneratedArtifact {
            method: "Ode(Rk4)".into(),
            source: "pub fn rk4_step() {}".into(),
            correlation_id: Some("corr-1".into()),
            source_document_id: Some("doc-1".into()),
        };
        let json = serde_json::to_string(&artifact).expect("serialise");
        assert_eq!(artifact, serde_json::from_str(&json).expect("deserialise"));
        let toml_str = toml::to_string(&artifact).expect("toml serialise");
        assert_eq!(
            artifact,
            toml::from_str(&toml_str).expect("toml deserialise")
        );
    }

    #[test]
    fn generated_artifact_bare_link_round_trips() {
        let artifact = GeneratedArtifact {
            method: "Root(Brent)".into(),
            source: String::new(),
            correlation_id: None,
            source_document_id: None,
        };
        let json = serde_json::to_string(&artifact).expect("serialise");
        assert_eq!(artifact, serde_json::from_str(&json).expect("deserialise"));
    }
}
