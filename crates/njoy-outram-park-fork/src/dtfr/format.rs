// Ported from NJOY2016 `src/dtfr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! DTF card-image line formatting (`subroutine dtfout`, `dtfr.f90:769-946`).
//!
//! Two output styles are modelled here:
//!
//! * **Format 0** (`iedit==0`, `dtfr.f90:787-807`) — the in-table DTF layout: a
//!   header line naming the table, followed by the whole `sig` array printed six
//!   values per line in `1p,6e12.4` format ([`format0_header`],
//!   [`format0_body`]).
//! * **Format 1** (`iedit==1`, td6/CLAW, `dtfr.f90:810-938`) — separate blocks
//!   with a trailing `hisnam`/name/sequence label on each line, six values per
//!   line in `1p,6e12.5` with the final partial line zero-filled
//!   ([`pack_dtf_block`]).
//!
//! The numeric values come from an in-memory [`crate::dtfr::table::DtfTable`];
//! the GENDF reader that fills it is not ported (see [`crate::dtfr::run`]). The
//! Fortran `e`-field byte layout (exponent sign, 2-digit exponent, column
//! padding) is reproduced by [`fortran_e`]; exact byte-for-byte agreement with a
//! specific Fortran compiler's `write` is a validation-pass concern (see the
//! module README).

/// Format a real as a Fortran `1p,eW.D` field: a mantissa in `[1,10)` (or
/// `-(1,10]`), an uppercase `E`, an explicit exponent sign, and a ≥2-digit
/// exponent, right-justified in `width` columns (`dtfr.f90:805` uses `e12.4`,
/// the block writers `e12.5`).
///
/// Example: `fortran_e(12345.6, 4, 12)` → `"  1.2346E+04"`.
pub fn fortran_e(x: f64, decimals: usize, width: usize) -> String {
    // Rust "{:.*E}" gives a [1,10) mantissa and an exponent with no leading
    // zeros and no '+' — reshape the exponent to Fortran's signed 2-digit form.
    let raw = format!("{:.*E}", decimals, x);
    let (mantissa, exp) = match raw.split_once('E') {
        Some((m, e)) => (m.to_string(), e.parse::<i32>().unwrap_or(0)),
        None => (raw, 0),
    };
    let sign = if exp < 0 { '-' } else { '+' };
    let field = format!("{}E{}{:02}", mantissa, sign, exp.abs());
    format!("{field:>width$}")
}

/// Header line for a **format-0** DTF table (`dtfr.f90:790-803`).
///
/// * `order` — the Legendre order `il` (neutron) or `ip` (photon).
/// * `ng` — number of neutron groups.
/// * `table_len` — `itabl` for a neutron table, `ngp` for a photon table.
/// * `matd`, `jz` — material number and sigma-zero index.
/// * `dtemp` — temperature in **kelvin**.
/// * `is_neutron` — `true` labels the line `il=`, `false` labels it `ip=`.
///
/// Reproduces the Fortran text, e.g.
/// `" il= 1 table 30 gp 76 pos, mat= 9237 iz= 1 temp= 2.93600E+02"`.
pub fn format0_header(
    order: i32,
    ng: i32,
    table_len: i32,
    matd: i32,
    jz: i32,
    dtemp: f64,
    is_neutron: bool,
) -> String {
    let tag = if is_neutron { "il" } else { "ip" };
    format!(
        " {tag}= {order} table{ng:3} gp{table_len:3} pos, mat={matd:5} iz={jz:2} temp={}",
        fortran_e(dtemp, 5, 12)
    )
}

/// Body of a **format-0** DTF table: the `sig` array, six values per line in
/// `1p,6e12.4` (`dtfr.f90:805`).
///
/// Unlike the block writer, format 0 does *not* zero-fill or label a short final
/// line — it simply prints `nw` values, six per line, so the last line may hold
/// fewer than six.
pub fn format0_body(sig: &[f64]) -> Vec<String> {
    sig.chunks(6)
        .map(|row| {
            let mut line = String::new();
            for &v in row {
                line.push_str(&fortran_e(v, 4, 12));
            }
            line
        })
        .collect()
}

/// Pack a **format-1** (td6/CLAW) block: six values per line in `1p,6e12.5`,
/// the final partial line zero-filled to six, each line carrying a trailing
/// `label` and an incrementing sequence number (`dtfr.f90:838-856, 873-898`).
///
/// * `values` — the column of group values to emit, in **barns**.
/// * `label` — the trailing tag (e.g. `hisnam` + reaction name), appended after
///   the six numeric fields.
///
/// The sequence number starts at 1 and increments once per emitted line
/// (`iseq`, `dtfr.f90:843`). Returns one string per line.
pub fn pack_dtf_block(values: &[f64], label: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut seq = 0;
    for row in values.chunks(6) {
        seq += 1;
        let mut line = String::new();
        for &v in row {
            line.push_str(&fortran_e(v, 5, 12));
        }
        // zero-fill the final short line to six fields (dtfr.f90:844-849)
        for _ in row.len()..6 {
            line.push_str(&fortran_e(0.0, 5, 12));
        }
        line.push_str(label);
        line.push_str(&seq.to_string());
        lines.push(line);
    }
    lines
}

/// Extract a single table column (fixed `jpos`, all groups) from a flat `sig`
/// array laid out as `sig(jpos + itabl*(jg-1))` (`dtfr.f90:838-841`).
///
/// Returns the `ng` values for DTF groups `1..=ng` at position `jpos`, ready to
/// hand to [`pack_dtf_block`].
pub fn column(sig: &[f64], jpos: i32, itabl: i32, ng: i32) -> Vec<f64> {
    (1..=ng)
        .map(|jg| {
            let locs = (jpos - 1) + itabl * (jg - 1);
            sig.get(locs as usize).copied().unwrap_or(0.0)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `fortran_e` reproduces the signed 2-digit-exponent `eW.D` field.
    ///
    /// **Methodology.** Fortran `1p,e12.4` prints a `[1,10)` mantissa, uppercase
    /// `E`, explicit exponent sign, ≥2-digit exponent, right-justified in 12
    /// cols. Check a positive, a negative, a sub-unity, and zero.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** `12345.6 → "  1.2346E+04"`;
    /// `-0.05 → " -5.0000E-02"`; `2.936e2 → "  2.9360E+02"`; `0 →
    /// "  0.0000E+00"`. Each is 12 chars wide.
    #[test]
    fn fortran_e_field() {
        assert_eq!(fortran_e(12345.6, 4, 12), "  1.2346E+04");
        assert_eq!(fortran_e(-0.05, 4, 12), " -5.0000E-02");
        assert_eq!(fortran_e(293.6, 4, 12), "  2.9360E+02");
        assert_eq!(fortran_e(0.0, 4, 12), "  0.0000E+00");
        assert_eq!(fortran_e(12345.6, 4, 12).len(), 12);
    }

    /// Format-0 header text and body line count.
    ///
    /// **Methodology.** `dtfr.f90:790-805`. Header names `il`, `ng`, `itabl`,
    /// `matd`, `jz`, `dtemp`; body prints `nw` values six per line with no
    /// zero-fill. For `nw=15` there must be 3 lines (6+6+3).
    ///
    /// **Result (2026-07-15).** Header contains `il= 1`, `mat= 9237`, `iz= 1`,
    /// and the temperature field `2.93600E+02`; a 15-value body yields 3 lines,
    /// the last holding 3 fields (36 chars) versus 72 for a full line.
    #[test]
    fn format0_header_and_body() {
        let h = format0_header(1, 30, 76, 9237, 1, 293.6, true);
        assert!(h.contains("il= 1"));
        assert!(h.contains("mat= 9237"));
        assert!(h.contains("iz= 1"));
        assert!(h.contains("2.93600E+02"));

        let sig: Vec<f64> = (0..15).map(|i| i as f64).collect();
        let body = format0_body(&sig);
        assert_eq!(body.len(), 3);
        assert_eq!(body[0].len(), 72); // 6 * 12
        assert_eq!(body[2].len(), 36); // 3 * 12, no zero-fill in format 0
    }

    /// Format-1 block: zero-fill of the short final line and sequence numbering.
    ///
    /// **Methodology.** `dtfr.f90:838-856`. Eight values → two lines: line 1
    /// full (6 fields), line 2 has 2 real fields + 4 zero-filled, both lines
    /// tagged with the label and a sequence number 1 then 2.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** 2 lines; both numeric parts are
    /// 72 chars (6×12) after zero-fill; line 1 ends with `label1`, line 2 with
    /// `label2`; the last four fields of line 2 are `0.00000E+00`.
    #[test]
    fn pack_block_zero_fill_and_sequence() {
        let values: Vec<f64> = (1..=8).map(|i| i as f64).collect();
        let lines = pack_dtf_block(&values, "u8els");
        assert_eq!(lines.len(), 2);
        // numeric part is first 72 chars
        assert_eq!(&lines[0][..72].len(), &72);
        assert!(lines[0].ends_with("u8els1"));
        assert!(lines[1].ends_with("u8els2"));
        // the 3rd..6th fields of line 2 are zero-filled
        let zero = fortran_e(0.0, 5, 12);
        assert!(lines[1][..72].contains(&zero));
        // exactly four zero fields at the tail of the numeric portion
        let numeric = &lines[1][..72];
        let zeros = numeric.matches(&zero).count();
        assert_eq!(zeros, 4);
    }

    /// `column` extracts a table position across all groups.
    ///
    /// **Methodology.** `sig(jpos + itabl*(jg-1))` (`dtfr.f90:838-841`). Build a
    /// tiny `itabl=4, ng=3` table where cell `(jpos,jg)` holds `10*jg + jpos`;
    /// extract position 2 → the three groups' values.
    ///
    /// **Result (2026-07-15).** column(pos=2) = `[12, 22, 32]`.
    #[test]
    fn column_extraction() {
        let (itabl, ng) = (4, 3);
        let mut sig = vec![0.0; (itabl * ng) as usize];
        for jg in 1..=ng {
            for jpos in 1..=itabl {
                let locs = (jpos - 1) + itabl * (jg - 1);
                sig[locs as usize] = (10 * jg + jpos) as f64;
            }
        }
        assert_eq!(column(&sig, 2, itabl, ng), vec![12.0, 22.0, 32.0]);
    }
}
