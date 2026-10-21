//! **Every review-wizard source cites a real standard-corpus document**
//! (GitHub #769).
//!
//! Methodology: `kovan-common` embeds the wizard's question set
//! (`data/review_wizard.toml`) but cannot see the standard corpus, which
//! lives in `kovan::corpus`. This test checks each `[[question.source]]`
//! `document` against `kovan::corpus::LITERATURE` ids, so a citation to a
//! document the corpus does not hold fails here.
//!
//! Result (2026-10-07): passes; 5 documents cited (10cfr50, doe-g-414.1-4,
//! doe-std-1172-2003, nureg-br-0167, nureg-km-0006), all in the corpus.

use kovan::corpus::LITERATURE;
use kovan_common::review::wizard::ReviewWizard;

#[test]
fn every_wizard_source_is_a_standard_corpus_document() {
    let cited = ReviewWizard::embedded().cited_documents();
    assert!(!cited.is_empty());
    for id in &cited {
        assert!(
            LITERATURE.iter().any(|l| l.id == *id),
            "review_wizard.toml cites {id:?}, which is not in kovan::corpus::LITERATURE"
        );
    }
}
