// Ported from NJOY2016 `src/acecm.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine newsuff`, l.619-684: replace the suffix of an ACE ZAID.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `newsuff`: the ZAID suffix edit ACER `iopt = 7` applies to a named
//! (thermal) table when card 2's `suff >= 0` (`aceth.f90:618`, gh:#1369,
//! upstream test 61).

/// `newsuff(mcnpx, suff, hz)` on the ZAID field `hz` as read (10 characters,
/// or 13 with `mcnpx`). Returns the new field, the same length.
///
/// Line for line:
/// - `lenhz = len_trim(hz)`, `indx = index(hz, ".", back)`;
/// - `indx < 1 .or. indx > 7`: "zaid name is nonstandard, no change made",
///   the field is returned unchanged;
/// - the suffix `nint(100*suff)` (`i2.2`) or `nint(1000*suff)` (`i3.3`,
///   `mcnpx`) is written after the period;
/// - when `lenhz < 10` (12 for `mcnpx`) the string is pushed right so the
///   class letter lands in column 10 (12), and the leading columns blanked.
///
/// Upstream's `hz` is `character(13)`; a non-mcnpx read fills `hz(1:10)`
/// only, so its `len_trim` is taken here with blanks after column 10.
#[must_use]
pub fn newsuff(mcnpx: bool, suff: f64, hz_in: &str) -> String {
    let mut hz: Vec<u8> = format!("{hz_in:<13.13}").into_bytes();
    let out_len = hz_in.len().clamp(if mcnpx { 13 } else { 10 }, 13);
    let lenhz = hz.iter().rposition(|&c| c != b' ').map_or(0, |p| p + 1);
    let Some(p) = hz.iter().rposition(|&c| c == b'.') else {
        return hz_in.to_string();
    };
    let indx = p + 1; // 1-based, as `index` returns it
    if indx > 7 {
        return hz_in.to_string();
    }
    let s = if mcnpx {
        format!("{:03}", (1000.0 * suff).round() as i64)
    } else {
        format!("{:02}", (100.0 * suff).round() as i64)
    };
    for (k, b) in s.bytes().enumerate() {
        hz[indx + k] = b; // hz(indx+1+k), 0-based
    }
    let indx = if mcnpx { 12 } else { 10 };
    if lenhz < indx {
        let idiff = indx - lenhz;
        // do i=indx,indx-lenhz+1,-1: hz(i:i)=hz(i-idiff:i-idiff)
        let mut i = indx;
        while i + lenhz > indx {
            hz[i - 1] = hz[i - idiff - 1];
            i -= 1;
        }
        for c in hz.iter_mut().take(idiff) {
            *c = b' ';
        }
    }
    String::from_utf8_lossy(&hz[..out_len]).to_string()
}

#[cfg(test)]
mod tests {
    use super::newsuff;

    /// Upstream test 61: `  hh2o.99t` with `suff = 0.90`.
    #[test]
    fn suffix_replaced_in_place() {
        assert_eq!(newsuff(false, 0.90, "  hh2o.99t"), "  hh2o.90t");
        assert_eq!(newsuff(true, 0.123, "  hh2o.999nt "), "  hh2o.123nt ");
    }

    /// A name shorter than the field is pushed right so the class letter is
    /// in column 10; a period beyond column 7 leaves the ZAID unchanged.
    #[test]
    fn short_name_is_right_shifted_and_nonstandard_kept() {
        assert_eq!(newsuff(false, 0.2, "lwtr.10t  "), "  lwtr.20t");
        assert_eq!(newsuff(false, 0.2, "abcdefgh.t"), "abcdefgh.t");
    }
}
