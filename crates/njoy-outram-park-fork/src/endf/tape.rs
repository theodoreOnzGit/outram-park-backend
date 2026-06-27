//! ENDF tape: an ordered collection of sections indexed by (MAT, MF, MT).
//!
//! Ported from the tape-management logic in NJOY2016 `endf.f90` (`findf`,
//! `tosend`, `tofend`, `tomend`, `totend`) and the Fortran logical-unit
//! abstraction in `util.f90` (`repoz`, `openz`).
//!
//! Unlike Fortran NJOY (which uses a global logical-unit number and physical
//! file), the Rust model owns the tape data in memory. A [`Tape`] is fully
//! parsed on construction; individual sections are accessed by key.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};

use crate::endf::parse::parse_line;
use crate::endf::EndfKey;
use crate::NjoyError;

/// The parsed data rows of one ENDF section (MAT, MF, MT).
///
/// Rows are the 6-field data lines that belong to this section, in file order.
/// Sentinel lines (SEND/FEND/MEND/TEND and the tape-ID line) are excluded.
#[derive(Debug, Clone)]
pub struct Section {
    /// Section identity.
    pub key: EndfKey,
    /// Data rows: one `[f64; 6]` per non-sentinel ENDF line in the section.
    pub rows: Vec<[f64; 6]>,
}

/// An ENDF tape parsed into sections.
///
/// Construct with [`Tape::read`]; look up sections by `(MAT, MF, MT)` via
/// [`Tape::section`] or iterate over all sections in file order via
/// [`Tape::sections`].
///
/// ## Example
/// ```no_run
/// use njoy_outram_park_fork::endf::tape::Tape;
///
/// let tape = Tape::read(std::fs::File::open("he4.endf").unwrap()).unwrap();
/// let sec = tape.section(228, 3, 2).unwrap();  // He-4 elastic cross section
/// println!("{} rows in MF=3 MT=2", sec.rows.len());
/// ```
#[derive(Debug)]
pub struct Tape {
    /// First line of the tape (free-text tape identification, MAT=1 MF=0 MT=0).
    pub tpid: String,
    /// All sections in file order.
    sections: Vec<Section>,
    /// Fast lookup by key.
    index: HashMap<EndfKey, usize>,
}

impl Tape {
    /// Parse an ENDF ASCII tape from any [`Read`] source.
    ///
    /// Line length must be 80 characters (padded with spaces if shorter is fine).
    /// Binary (blocked-binary) tapes are not supported in this version.
    pub fn read<R: Read>(reader: R) -> Result<Self, NjoyError> {
        let mut lines = BufReader::new(reader).lines();
        let mut tpid = String::new();
        let mut sections: Vec<Section> = Vec::new();
        let mut index: HashMap<EndfKey, usize> = HashMap::new();

        // The very first line is the tape identification record (TPID).
        // It has MAT=1 (or 0), MF=0, MT=0 but the data is 17 × 4-char Hollerith —
        // we store it as-is for now and skip the column-66 MAT/MF/MT parsing.
        if let Some(first) = lines.next() {
            tpid = first.map_err(NjoyError::Io)?;
        }

        let mut current_key: Option<EndfKey> = None;
        let mut current_rows: Vec<[f64; 6]> = Vec::new();

        for line_res in lines {
            let line = line_res.map_err(NjoyError::Io)?;
            let rl = parse_line(&line)?;

            // Sentinel detection: MT=0 (SEND), MF=0 (FEND), MAT=0 (MEND), MAT=-1 (TEND)
            let is_sentinel = rl.mt == 0 || rl.mf == 0 || rl.mat == 0 || rl.mat == -1;

            if is_sentinel {
                // Flush any open section
                if let Some(key) = current_key.take() {
                    let idx = sections.len();
                    sections.push(Section { key, rows: std::mem::take(&mut current_rows) });
                    index.entry(key).or_insert(idx);
                }
                continue;
            }

            let key = EndfKey { mat: rl.mat, mf: rl.mf, mt: rl.mt };

            // Start a new section when the key changes
            if current_key != Some(key) {
                if let Some(prev_key) = current_key.take() {
                    let idx = sections.len();
                    sections.push(Section { key: prev_key, rows: std::mem::take(&mut current_rows) });
                    index.entry(prev_key).or_insert(idx);
                }
                current_key = Some(key);
            }

            current_rows.push(rl.fields);
        }

        // Flush the last open section (if the tape lacked a TEND)
        if let Some(key) = current_key.take() {
            let idx = sections.len();
            sections.push(Section { key, rows: std::mem::take(&mut current_rows) });
            index.entry(key).or_insert(idx);
        }

        Ok(Tape { tpid, sections, index })
    }

    /// Look up a section by `(mat, mf, mt)`, returning `None` if absent.
    pub fn section(&self, mat: i32, mf: i32, mt: i32) -> Option<&Section> {
        let key = EndfKey { mat, mf, mt };
        self.index.get(&key).map(|&i| &self.sections[i])
    }

    /// Iterate over all sections in file order.
    pub fn sections(&self) -> &[Section] {
        &self.sections
    }

    /// Number of sections (excluding sentinel/tpid lines).
    pub fn len(&self) -> usize {
        self.sections.len()
    }

    /// True if the tape contains no data sections.
    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }
}
