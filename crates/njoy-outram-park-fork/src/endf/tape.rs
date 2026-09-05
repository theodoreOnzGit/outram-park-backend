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
use std::io::{BufRead, BufReader, Read, Write};

use crate::endf::parse::{format_line, parse_line};
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
    /// Parse an ENDF ASCII tape from a file on disk.
    ///
    /// [`Tape::read`] is generic over [`Read`], which is right for Rust and
    /// unreachable from a binding generator that cannot monomorphise a type
    /// parameter. This is the same parse behind a concrete signature, so
    /// `Tape::read_file("n-094_Pu_239.endf")` works from Rust and from Python
    /// alike -- the ordinary case, without the caller opening the file first.
    ///
    /// # Errors
    ///
    /// [`NjoyError::Io`] if the file cannot be opened or read, or any parse
    /// error [`Tape::read`] reports.
    /// ```no_run
    /// use njoy_outram_park_fork::endf::tape::Tape;
    /// use std::path::Path;
    ///
    /// let tape = Tape::read_file(Path::new("n-092_U_238.endf"))?;
    /// let mat = tape.materials()[0];          // the tape knows its own MAT
    /// let mf3_total = tape.section(mat, 3, 1); // MF=3, MT=1: total cross section
    /// # Ok::<(), njoy_outram_park_fork::NjoyError>(())
    /// ```
    ///
    /// Prefer this over `File::open` + [`Tape::read`] for a file on disk. The
    /// generic [`Tape::read`] is for the cases this cannot serve — a socket, a
    /// decompressor, an in-memory buffer.
    pub fn read_file(path: &std::path::Path) -> Result<Self, NjoyError> {
        let file = std::fs::File::open(path).map_err(NjoyError::Io)?;
        Self::read(file)
    }

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
                    sections.push(Section {
                        key,
                        rows: std::mem::take(&mut current_rows),
                    });
                    index.entry(key).or_insert(idx);
                }
                continue;
            }

            let key = EndfKey {
                mat: rl.mat,
                mf: rl.mf,
                mt: rl.mt,
            };

            // Start a new section when the key changes
            if current_key != Some(key) {
                if let Some(prev_key) = current_key.take() {
                    let idx = sections.len();
                    sections.push(Section {
                        key: prev_key,
                        rows: std::mem::take(&mut current_rows),
                    });
                    index.entry(prev_key).or_insert(idx);
                }
                current_key = Some(key);
            }

            current_rows.push(rl.fields);
        }

        // Flush the last open section (if the tape lacked a TEND)
        if let Some(key) = current_key.take() {
            let idx = sections.len();
            sections.push(Section {
                key,
                rows: std::mem::take(&mut current_rows),
            });
            index.entry(key).or_insert(idx);
        }

        Ok(Tape {
            tpid,
            sections,
            index,
        })
    }

    /// Look up a section by `(mat, mf, mt)`, returning `None` if absent.
    pub fn section(&self, mat: i32, mf: i32, mt: i32) -> Option<&Section> {
        let key = EndfKey { mat, mf, mt };
        self.index.get(&key).map(|&i| &self.sections[i])
    }

    /// Iterate over all sections in file order.
    /// Every ENDF material number on this tape, ascending and deduplicated.
    ///
    /// A tape carries its own MAT numbers, so a caller should never have to
    /// look one up in a table to use the file they already hold. Most
    /// evaluations contain exactly one material, which makes
    /// `tape.materials()[0]` the common case.
    #[must_use]
    pub fn materials(&self) -> Vec<i32> {
        let mut mats: Vec<i32> = self.sections.iter().map(|s| s.key.mat).collect();
        mats.sort_unstable();
        mats.dedup();
        mats
    }

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

    /// Construct a tape from an explicit tape-ID line and section list, in
    /// file order. Used by material-selection logic (e.g. `crate::moder`) to
    /// assemble a new tape from sections drawn out of one or more inputs.
    pub fn from_sections(tpid: String, sections: Vec<Section>) -> Self {
        let mut index = HashMap::new();
        for (i, sec) in sections.iter().enumerate() {
            index.entry(sec.key).or_insert(i);
        }
        Tape {
            tpid,
            sections,
            index,
        }
    }

    /// Write this tape back out in ENDF ASCII (formatted) mode — the write
    /// side of [`Tape::read`], and the core of what NJOY's **MODER** module
    /// does when converting mode (`endf.f90`'s `contio`/`lineio` write paths,
    /// sequenced the way `moder.f90` walks a tape: emit each section's data
    /// rows, then the sentinel records marking section/file/material/tape
    /// boundaries).
    ///
    /// Sentinel emission mirrors the table in this module's docs: a **SEND**
    /// (`MT=0`) always follows a section's data rows; a **FEND** (`MF=0`)
    /// follows when the next section changes `MF` or `MAT` (or there is no
    /// next section); a **MEND** (`MAT=0`) follows when the next section
    /// changes `MAT` (or there is no next section); a final **TEND**
    /// (`MAT=-1`) always closes the tape.
    ///
    /// **Known simplifications** (see [`crate::endf::parse::format_line`] for
    /// the CONT-vs-LIST field-encoding caveat):
    /// - Only ASCII (formatted) output is produced; there is no NJOY
    ///   blocked-binary writer (see `src/moder/README.md` — a fully in-memory
    ///   Rust pipeline does not need one).
    /// - `format_line` writes every row's six fields in exponential `a11`
    ///   form rather than NJOY's plain right-justified `i11` integers for
    ///   CONT records' L1/L2/N1/N2 fields — the written *value* round-trips
    ///   exactly through this crate's own reader, but the column layout does
    ///   not byte-match genuine NJOY output for those four fields.
    /// - Sequence numbers (columns 76-80) are a simple monotonically
    ///   increasing counter across the whole tape, not NJOY's per-file
    ///   `nsh`/`nsp`/`nsc` reset convention — `parse_line` (and every ENDF
    ///   reader) documents this column as cosmetic/ignored on read.
    pub fn write<W: Write>(&self, mut w: W) -> Result<(), NjoyError> {
        writeln!(w, "{}", self.tpid).map_err(NjoyError::Io)?;

        let mut seq: i32 = 1;
        let mut iter = self.sections.iter().peekable();
        while let Some(sec) = iter.next() {
            for row in &sec.rows {
                writeln!(
                    w,
                    "{}",
                    format_line(row, sec.key.mat, sec.key.mf, sec.key.mt, seq)
                )
                .map_err(NjoyError::Io)?;
                seq += 1;
            }

            // SEND — end of section (always follows a section's data rows).
            writeln!(
                w,
                "{}",
                format_line(&[0.0; 6], sec.key.mat, sec.key.mf, 0, seq)
            )
            .map_err(NjoyError::Io)?;
            seq += 1;

            let next_key = iter.peek().map(|s| s.key);
            let file_ends = next_key.is_none_or(|k| k.mf != sec.key.mf || k.mat != sec.key.mat);
            let material_ends = next_key.is_none_or(|k| k.mat != sec.key.mat);

            if file_ends {
                // FEND — end of file (MF=0).
                writeln!(w, "{}", format_line(&[0.0; 6], sec.key.mat, 0, 0, seq))
                    .map_err(NjoyError::Io)?;
                seq += 1;
            }
            if material_ends {
                // MEND — end of material (MAT=0).
                writeln!(w, "{}", format_line(&[0.0; 6], 0, 0, 0, seq)).map_err(NjoyError::Io)?;
                seq += 1;
            }
        }

        // TEND — end of tape (MAT=-1).
        writeln!(w, "{}", format_line(&[0.0; 6], -1, 0, 0, seq)).map_err(NjoyError::Io)?;
        Ok(())
    }
}
