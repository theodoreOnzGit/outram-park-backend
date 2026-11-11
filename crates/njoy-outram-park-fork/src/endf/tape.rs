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
///
/// `Clone` since 2026-10-14 (GitHub #871): [`crate::process::NuclearCrossSections`]
/// keeps the evaluation it was made from, for ACER.
#[derive(Debug, Clone)]
pub struct Tape {
    /// First line of the tape (free-text tape identification, MAT=1 MF=0 MT=0).
    pub tpid: String,
    /// All sections in file order.
    sections: Vec<Section>,
    /// Fast lookup by key.
    index: HashMap<EndfKey, usize>,
    /// The raw text (columns 1-66) of every row of the sections whose lines
    /// the six-float row parser cannot represent, one entry per row:
    ///
    /// - **MF=32**: the INTG records of a compact (`LCOMP=2`) covariance are
    ///   `2i5,1x,18i3`-style integer lines;
    /// - **MF=1/MT=451** (since 2026-10-05, GitHub #553): the evaluation's
    ///   descriptive text, which [`Tape::write`] writes back verbatim.
    ///
    /// Only these are kept so the tape's memory footprint does not double.
    raw_mf32: HashMap<EndfKey, Vec<String>>,
    /// Closing records NJOY *copies* from a scratch tape (`tosend`/`tofend`/
    /// `tomend`, `endf.f90:1064-1230`) instead of writing with `asend`/`afend`/
    /// `amend`: a copied record carries the running sequence number (and counts
    /// on) where a written one carries 99999 / 0 and restarts the count. Keys:
    /// a section's own key for its SEND, `{mat, mf, mt: 0}` for the FEND of
    /// `(mat, mf)`, `{mat, mf: 0, mt: 0}` for the MEND of `mat`. RECONR's
    /// `recout` copies every non-redundant MF=3 section and the files after
    /// it, so its PENDF numbers its lines this way (gh:#1484).
    copied_closers: std::collections::HashSet<EndfKey>,
}

/// Whether [`Tape::read`] keeps a section's raw line text (see `raw_mf32`).
fn keeps_raw(key: EndfKey) -> bool {
    key.mf == 32 || (key.mf == 1 && key.mt == 451)
}

/// Decode one ENDF record read as bytes (`\n` already removed) into a
/// string with one `char` per card column (gh:#660).
///
/// NJOY2016 reads each record with a fixed-width format,
/// `(6e11.0,i4,i2,i3,i5)` (`endf.f90:55`), so a column is a byte and the text
/// has no encoding. This reproduces that exactly for single-byte encodings:
/// bytes that are not valid UTF-8 (a Latin-1 tape) are mapped one byte to one
/// `char`. Valid UTF-8 is decoded to characters, so a multibyte character in
/// MF=1/MT=451 text counts as **one** column and MAT/MF/MT stay in columns
/// 67-75. NJOY would read that line's MAT/MF/MT from shifted bytes; here the
/// line is keyed as its author laid it out (the issue's guidance, gh:#660,
/// rather than mis-keying it). A trailing `\r` is dropped, as
/// `BufRead::lines` did before.
pub fn decode_endf_line(mut bytes: Vec<u8>) -> String {
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(e) => e.into_bytes().iter().map(|&b| char::from(b)).collect(),
    }
}

impl Tape {
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
    ///
    /// A gzipped file (the `1f 8b` magic bytes) is inflated first (since
    /// 2026-10-05), with the same pure-Rust decoder the ACE reader uses.
    pub fn read_file(path: &std::path::Path) -> Result<Self, NjoyError> {
        let bytes = std::fs::read(path).map_err(NjoyError::Io)?;
        if bytes.len() > 2 && bytes[0] == 0x1f && bytes[1] == 0x8b {
            let text = crate::acer::read::gunzip(&bytes, path)?;
            return Self::read(&text[..]);
        }
        Self::read(&bytes[..])
    }

    /// Parse an ENDF ASCII tape from any [`Read`] source.
    ///
    /// Line length must be 80 characters (padded with spaces if shorter is fine).
    /// Binary (blocked-binary) tapes are not supported in this version.
    /// (CORRECTED 2026-10-04: these three lines used to sit above
    /// [`Tape::read_file`], so rustdoc showed them on the wrong function.)
    ///
    /// Lines are read as bytes and decoded per line by [`decode_endf_line`]
    /// (since 2026-11-10, gh:#660): UTF-8 where valid, otherwise one byte per
    /// column (Latin-1), so a non-ASCII character in MF=1/MT=451 text neither
    /// fails the read nor panics the column slicing.
    pub fn read<R: Read>(reader: R) -> Result<Self, NjoyError> {
        let mut lines = BufReader::new(reader)
            .split(b'\n')
            .map(|r| r.map(decode_endf_line));
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
        let mut current_raw: Vec<String> = Vec::new();
        let mut raw_mf32: HashMap<EndfKey, Vec<String>> = HashMap::new();

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
                    if keeps_raw(key) {
                        raw_mf32
                            .entry(key)
                            .or_insert(std::mem::take(&mut current_raw));
                    }
                    current_raw.clear();
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
                    if keeps_raw(prev_key) {
                        raw_mf32
                            .entry(prev_key)
                            .or_insert(std::mem::take(&mut current_raw));
                    }
                    current_raw.clear();
                }
                current_key = Some(key);
            }

            current_rows.push(rl.fields);
            if keeps_raw(key) {
                // First 66 columns (characters, gh:#660), not bytes.
                current_raw.push(line.chars().take(66).collect());
            }
        }

        // Flush the last open section (if the tape lacked a TEND)
        if let Some(key) = current_key.take() {
            let idx = sections.len();
            sections.push(Section {
                key,
                rows: std::mem::take(&mut current_rows),
            });
            index.entry(key).or_insert(idx);
            if keeps_raw(key) {
                raw_mf32
                    .entry(key)
                    .or_insert(std::mem::take(&mut current_raw));
            }
        }

        Ok(Tape {
            tpid,
            sections,
            index,
            raw_mf32,
            copied_closers: Default::default(),
        })
    }

    /// Look up a section by `(mat, mf, mt)`, returning `None` if absent.
    pub fn section(&self, mat: i32, mf: i32, mt: i32) -> Option<&Section> {
        let key = EndfKey { mat, mf, mt };
        self.index.get(&key).map(|&i| &self.sections[i])
    }

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

    /// All sections in file order. (CORRECTED 2026-10-04: this line used to
    /// sit above [`Tape::materials`].)
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
            raw_mf32: HashMap::new(),
            copied_closers: Default::default(),
        }
    }

    /// Carry another tape's raw text over (MF=32 rows and MF=1/MT=451 text)
    /// to a tape rebuilt with [`Tape::from_sections`] from `other`'s sections
    /// (`errorr::covadd`, `moder::select_materials`).
    pub fn copy_raw_mf32_from(&mut self, other: &Tape) {
        for (k, v) in &other.raw_mf32 {
            self.raw_mf32.entry(*k).or_insert_with(|| v.clone());
        }
    }

    /// Set the raw text (columns 1-66, one entry per row) of a section whose
    /// lines the six-float rows cannot carry: MF=1/MT=451's description, for a
    /// PENDF a deck driver builds (gh:#1369, `deck::pendf`).
    pub fn set_raw_text(&mut self, key: EndfKey, lines: Vec<String>) {
        self.raw_mf32.insert(key, lines);
    }

    /// Mark closing records as copied, not written, by NJOY (see the
    /// `copied_closers` field): [`Tape::write`] then numbers them with the
    /// running sequence count.
    pub fn set_copied_closers(&mut self, keys: impl IntoIterator<Item = EndfKey>) {
        self.copied_closers.extend(keys);
    }

    /// Take over `other`'s copied-closer marks (see [`Tape::set_copied_closers`]),
    /// for a tape re-read from the text `other` wrote (the numbering is the
    /// only thing a re-read loses).
    pub fn copy_closers_from(&mut self, other: &Tape) {
        self.copied_closers.extend(other.copied_closers.iter().copied());
    }

    /// The raw text rows of `key` (see [`Tape::set_raw_text`]).
    pub fn raw_text(&self, key: EndfKey) -> Option<&[String]> {
        self.raw_mf32.get(&key).map(|v| v.as_slice())
    }

    /// The raw text (columns 1-66) of an MF=32 section's data rows, one
    /// string per row of [`Section::rows`] — `None` for a tape not read
    /// from text, or a section that is not MF=32.
    pub fn raw_mf32_lines(&self, mat: i32, mt: i32) -> Option<&[String]> {
        self.raw_mf32
            .get(&EndfKey { mat, mf: 32, mt })
            .map(|v| v.as_slice())
    }

    /// Assemble a minimal **PENDF** tape for one material from pointwise
    /// lin-lin MF=3 cross sections — the in-memory counterpart of what
    /// RECONR/BROADR write, sufficient for the modules that *read* a PENDF
    /// (GROUPR's `getsig`, ERRORR's `grpav`).
    ///
    /// Layout written:
    /// - MF=1/MT=451: HEAD `(za, awr, 0, 0, 0, 0)`; for `iverf >= 5` the
    ///   `(elis, sta, lis, liso, 0, nfor)` CONT; for `iverf >= 6` the
    ///   `(awi, emax, lrel, 0, nsub, nver)` CONT; then the `hdatio` head
    ///   `(temp_k, 0, 1, 0, 0, 0)` — the record NJOY reads the material
    ///   temperature from (`groupr.f90`/`errorr.f90` `hdatio` + `c1h`).
    /// - MF=3/MT for each `(mt, pairs)`: HEAD `(za, awr, 0, 0, 0, 0)` + a
    ///   one-region lin-lin TAB1 `(0, 0, 0, 0, 1, np) / (np, 2) / pairs`.
    ///
    /// No MF=2 is written (no resonance range record), so a consumer that
    /// needs `thnmax`/URR tables must not rely on this tape for them.
    pub fn pendf_from_pointwise<'a>(
        mat: i32,
        iverf: i32,
        za: f64,
        awr: f64,
        temp_k: f64,
        sections: impl IntoIterator<Item = (i32, &'a [(f64, f64)])>,
    ) -> Self {
        let key = |mf: i32, mt: i32| EndfKey { mat, mf, mt };
        let mut out = Vec::new();
        let mut rows = vec![[za, awr, 0.0, 0.0, 0.0, 0.0]];
        if iverf >= 5 {
            rows.push([0.0, 0.0, 0.0, 0.0, 0.0, f64::from(iverf.min(6))]);
        }
        if iverf >= 6 {
            rows.push([1.0, 2.0e7, 0.0, 0.0, 10.0, 8.0]);
        }
        rows.push([temp_k, 0.0, 1.0, 0.0, 0.0, 0.0]);
        out.push(Section {
            key: key(1, 451),
            rows,
        });
        for (mt, pairs) in sections {
            let np = pairs.len() as f64;
            let mut rows = vec![
                [za, awr, 0.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, 0.0, 0.0, 1.0, np],
                [np, 2.0, 0.0, 0.0, 0.0, 0.0],
            ];
            for chunk in pairs.chunks(3) {
                let mut row = [0.0f64; 6];
                for (i, &(x, y)) in chunk.iter().enumerate() {
                    row[2 * i] = x;
                    row[2 * i + 1] = y;
                }
                rows.push(row);
            }
            out.push(Section {
                key: key(3, mt),
                rows,
            });
        }
        Tape::from_sections(String::new(), out)
    }

    /// Write this tape back out in ENDF ASCII (formatted) mode — the write
    /// side of [`Tape::read`], and the core of what NJOY's **MODER** module
    /// does when converting mode.
    ///
    /// Each section's lines are written as MODER writes them (GitHub #553):
    /// [`crate::moder`]'s record-layout port (`moder.f90`'s `file1` …
    /// `file40`) says which line is a CONT head, data, an interpolation
    /// table, MF=1/MT=451 text or a directory entry, and each is formatted
    /// with `endf.f90`'s own edit descriptors:
    ///
    /// - CONT: `C1, C2` as `a11`, `L1, L2, N1, N2` as `i11` (`contio`);
    /// - data: `a11`, with the fields past a record's count left blank
    ///   (`lineio`);
    /// - interpolation tables: `i11` pairs, the rest blank (`tablio`);
    /// - MF=1/MT=451 text: the 66 columns read from the input tape, verbatim
    ///   (`hdatio`); blank for a tape not read from text;
    /// - directory: 22 blanks and four `i11` (`dictio`).
    ///
    /// Sentinels are blank in columns 1-66, as `asend`/`afend`/`amend`/
    /// `atend` write them, and the sequence number follows NJOY: each section
    /// counts from 1, a SEND carries 99999, a FEND/MEND/TEND carries 0, and
    /// the count wraps from 99999 to 1.
    ///
    /// **Not byte-faithful yet:** MF=32 (resonance covariances; MODER's
    /// `file32` is not ported), GENDF and ERRORR-output materials (which
    /// MODER copies through a separate path), any section whose records do
    /// not walk to its last row, and any section where the layout would
    /// blank a non-zero field, are written with every field as `a11`, as
    /// before. A value is never dropped to match a format.
    ///
    /// Only ASCII (formatted) output is produced; there is no NJOY
    /// blocked-binary writer (see `src/moder/README.md` — a fully in-memory
    /// Rust pipeline does not need one).
    ///
    /// ~~**Measured against NJOY2016's MODER** (2026-10-05, GitHub #536 …):
    /// only 1 line is byte-identical~~ **CHANGED 2026-10-05 (#553):** see
    /// `tests/moder_vs_njoy2016.rs` for the current comparison.
    pub fn write<W: Write>(&self, mut w: W) -> Result<(), NjoyError> {
        use crate::endf::parse::format_endf_float;
        use crate::moder::layout::{
            is_group_material, iverf_from_mf1, layout_keeps_every_value, section_layout, LineKind,
        };

        let io = NjoyError::Io;
        let blank66 = " ".repeat(66);
        let i11 = |x: f64| format!("{:>11}", x.round() as i64);
        writeln!(w, "{}", self.tpid).map_err(io)?;

        let mut seq: i32 = 1;
        let mut iverf = 6;
        let mut group_material = false;
        let mut iter = self.sections.iter().peekable();
        while let Some(sec) = iter.next() {
            let k = sec.key;
            if k.mf == 1 && k.mt == 451 {
                // `moder.f90:184-195`: the format version, and `nsh = 1`.
                iverf = iverf_from_mf1(&sec.rows);
                group_material = is_group_material(&sec.rows);
                seq = 1;
            }
            let layout = if group_material {
                None
            } else {
                section_layout(k.mf, k.mt, &sec.rows, iverf).filter(|l| layout_keeps_every_value(&sec.rows, l))
            };
            let raw = self.raw_mf32.get(&k);
            for (i, row) in sec.rows.iter().enumerate() {
                let body: String = match layout.as_ref().map(|l| l[i]) {
                    Some(LineKind::Cont) => {
                        let mut b = format_endf_float(row[0]);
                        b.push_str(&format_endf_float(row[1]));
                        for &x in &row[2..] {
                            b.push_str(&i11(x));
                        }
                        b
                    }
                    Some(LineKind::Data(n)) => (0..6)
                        .map(|j| if j < n as usize { format_endf_float(row[j]) } else { " ".repeat(11) })
                        .collect(),
                    Some(LineKind::Ints(n)) => {
                        (0..6).map(|j| if j < n as usize { i11(row[j]) } else { " ".repeat(11) }).collect()
                    }
                    Some(LineKind::Text) => match raw.and_then(|r| r.get(i)) {
                        Some(t) => format!("{t:<66.66}"),
                        None => blank66.clone(),
                    },
                    Some(LineKind::Dir) => {
                        let mut b = " ".repeat(22);
                        for &x in &row[2..] {
                            b.push_str(&i11(x));
                        }
                        b
                    }
                    None => row.iter().map(|&x| format_endf_float(x)).collect(),
                };
                writeln!(w, "{body}{:4}{:2}{:3}{:5}", k.mat, k.mf, k.mt, seq).map_err(io)?;
                seq = if seq >= 99_999 { 1 } else { seq + 1 };
            }

            // SEND (`asend`): sequence 99999, then the next section starts at 1.
            if self.copied_closers.contains(&k) {
                // `tosend`: copied with the running count, which goes on.
                writeln!(w, "{blank66}{:4}{:2}{:3}{:5}", k.mat, k.mf, 0, seq).map_err(io)?;
                seq = if seq >= 99_999 { 1 } else { seq + 1 };
            } else {
                writeln!(w, "{blank66}{:4}{:2}{:3}{:5}", k.mat, k.mf, 0, 99_999).map_err(io)?;
                seq = 1;
            }

            let next_key = iter.peek().map(|s| s.key);
            // A material repeats on a multi-temperature PENDF: its next
            // occurrence starts at MF=1/MT=451, and the previous one is
            // closed with FEND and MEND, as on NJOY's tapes (since
            // 2026-10-05; before, the two temperatures ran together).
            let new_occurrence = |n: crate::endf::EndfKey| n.mf == 1 && n.mt == 451;
            if next_key.is_none_or(|n| n.mf != k.mf || n.mat != k.mat || new_occurrence(n)) {
                // FEND (`afend`): sequence 0.
                if self.copied_closers.contains(&EndfKey { mat: k.mat, mf: k.mf, mt: 0 }) {
                    writeln!(w, "{blank66}{:4}{:2}{:3}{:5}", k.mat, 0, 0, seq).map_err(io)?;
                    seq = if seq >= 99_999 { 1 } else { seq + 1 };
                } else {
                    writeln!(w, "{blank66}{:4}{:2}{:3}{:5}", k.mat, 0, 0, 0).map_err(io)?;
                    seq = 1;
                }
            }
            if next_key.is_none_or(|n| n.mat != k.mat || new_occurrence(n)) {
                // MEND (`amend`).
                if self.copied_closers.contains(&EndfKey { mat: k.mat, mf: 0, mt: 0 }) {
                    writeln!(w, "{blank66}{:4}{:2}{:3}{:5}", 0, 0, 0, seq).map_err(io)?;
                    seq = if seq >= 99_999 { 1 } else { seq + 1 };
                } else {
                    writeln!(w, "{blank66}{:4}{:2}{:3}{:5}", 0, 0, 0, 0).map_err(io)?;
                    seq = 1;
                }
            }
        }

        // TEND (`atend`).
        writeln!(w, "{blank66}{:4}{:2}{:3}{:5}", -1, 0, 0, 0).map_err(io)?;
        Ok(())
    }
}
