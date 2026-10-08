// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_integration.js
// Version:     2.4.63, commit 73bc1b44bc7d54d0bfec4e070fd27f5efe024ff9
// Copyright:   (c) 2009-2019 Frank Bennett
// Licence:     AGPL-3.0, taken from upstream's "CPAL-1.0 or AGPL-3.0-or-later"
//              (LICENSE at the commit above; see this crate's NOTICE).
// Modified:    2026-10-08, by the OUTRAM PARK contributors. This file is a
//              Rust translation (port) of the files named above, modified
//              from the original.
// No warranty: this program is distributed in the hope that it will be
//              useful, but WITHOUT ANY WARRANTY; without even the implied
//              warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
//              PURPOSE. See the GNU Affero General Public License.

//! `CSL.Engine.prototype.setCitationId` (util_integration.js): give a citation
//! a unique `citationID` and register it in `citationById`.

use super::disambig_citations::CitId;
use super::state::State;

/// `a` followed by `id` in base 32 (`"a" + id.toString(32)`).
fn base32(mut id: u64) -> String {
    const DIGITS: &[u8; 32] = b"0123456789abcdefghijklmnopqrstuv";
    if id == 0 {
        return "0".to_string();
    }
    let mut out = Vec::new();
    while id > 0 {
        out.push(DIGITS[(id % 32) as usize]);
        id /= 32;
    }
    out.reverse();
    String::from_utf8_lossy(&out).into_owned()
}

impl State {
    /// `setCitationId(citation, force)`: unless `citation` has a `citationID`
    /// (or `force`), give it a fresh one; then register it in
    /// `registry.citationreg.citationById`. Returns what upstream returns:
    /// `false` (`None`) when the ID was kept, else the number drawn.
    ///
    /// Upstream draws `Math.floor(Math.random() * 100000000000000)` and walks
    /// to a free number; here the number comes from a deterministic
    /// generator (the draw is not observable except through the ID).
    pub fn set_citation_id(&mut self, citation: CitId, force: bool) -> Option<String> {
        let mut ret = None;
        let has_id = !self.registry.citationreg.get(citation).citation_id.is_empty();
        if !has_id || force {
            // splitmix64 step over a counter held in the registry
            self.registry.citation_id_counter = self
                .registry
                .citation_id_counter
                .wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.registry.citation_id_counter;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^= z >> 31;
            let mut id: u64 = z % 100_000_000_000_000;
            loop {
                let direction;
                let candidate = format!("a{}", base32(id));
                if !self.registry.citationreg.citation_by_id.contains_key(&candidate) {
                    self.registry.citationreg.get_mut(citation).citation_id = candidate;
                    break;
                } else if id < 50_000_000_000_000 {
                    direction = 1;
                } else {
                    direction = -1;
                }
                if direction == 1 {
                    id += 1;
                } else {
                    id = id.saturating_sub(1);
                }
            }
            ret = Some(id.to_string());
        }
        let cid = self.registry.citationreg.get(citation).citation_id.clone();
        self.registry.citationreg.citation_by_id.insert(cid, citation);
        ret
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::citeproc::disambig_citations::CitationRec;

    #[test]
    fn ids_are_kept_or_generated_and_registered() {
        let mut s = State::default();
        let kept = s.registry.citationreg.alloc(CitationRec {
            citation_id: "C1".into(),
            ..CitationRec::default()
        });
        assert_eq!(s.set_citation_id(kept, false), None);
        assert_eq!(s.registry.citationreg.by_id("C1"), Some(kept));
        let fresh = s.registry.citationreg.alloc(CitationRec::default());
        let r = s.set_citation_id(fresh, false);
        assert!(r.is_some());
        let id = s.registry.citationreg.get(fresh).citation_id.clone();
        assert!(id.starts_with('a') && id.len() > 1);
        assert_eq!(s.registry.citationreg.by_id(&id), Some(fresh));
        // two fresh ids differ
        let other = s.registry.citationreg.alloc(CitationRec::default());
        s.set_citation_id(other, false);
        assert_ne!(s.registry.citationreg.get(other).citation_id, id);
        // force replaces an existing id
        s.set_citation_id(kept, true);
        assert_ne!(s.registry.citationreg.get(kept).citation_id, "C1");
        assert_eq!(base32(31), "v");
        assert_eq!(base32(32), "10");
    }
}
