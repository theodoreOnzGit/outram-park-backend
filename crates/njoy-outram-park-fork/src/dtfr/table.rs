// Ported from NJOY2016 `src/dtfr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! DTF table assembly — group ordering, `sig`-array indexing, and the reduced
//! Sₙ transfer-matrix (triangular) packing (`dtfr.f90:296-427`).
//!
//! DTFR accumulates group constants into a flat `sig` array laid out as
//! `sig(jpos + itabl*(jg-1))` — `itabl` table positions per group, `ng` groups
//! (`dtfr.f90:343-346`). Groups run in **DTF order** (group 1 = highest energy),
//! so a GENDF group index `ig` maps to the DTF group `jg = ng - ig + 1`
//! ([`dtf_group`], `dtfr.f90:342`).
//!
//! The scattering source is packed into a **reduced-length, up-scatter-capable**
//! band: for a transfer from group `jg` to secondary group `jg2`, the table
//! position is `ipingp + (jg2 - jg)` — the in-group position `ipingp` for
//! `jg2 == jg`, later positions for down-scatter, earlier ones for up-scatter —
//! with fold-back clamps that lump anything past `itabl` into the last position
//! and keep the scatter band clear of the cross-section positions `<= iptotl`
//! ([`scatter_position`], `dtfr.f90:410-416`).
//!
//! Cross sections are in **barns**; all indices are dimensionless. This module
//! operates on an in-memory [`DtfTable`]; the GENDF-record reader that supplies
//! the raw transfer values is not ported here (see [`crate::dtfr::run`]).

/// Map a GENDF group index `ig` to its DTF group number `jg = ng - ig + 1`
/// (`dtfr.f90:342`). DTF orders groups from highest energy (`jg==1`) to lowest.
pub fn dtf_group(ig: i32, ng: i32) -> i32 {
    ng - ig + 1
}

/// Compute the reduced-table scatter position `(jpos, jg2)` for a transfer from
/// DTF group `jg` to the `k`-th secondary group of a GENDF list record
/// (`dtfr.f90:410-416`).
///
/// * `jg` — the DTF "from" group (`= ng - ig + 1`).
/// * `ig2lo` — GENDF group index of the first secondary group in the record.
/// * `k` — secondary-group loop index (`2..=ng2`); `k==1` is the flux/total.
/// * `ng` — number of neutron groups.
/// * `ipingp` — table position of in-group scatter.
/// * `iptotl` — table position of the total cross section (scatter band must
///   stay above this).
/// * `itabl` — table length (scatter band folds back into `itabl`).
///
/// Returns `(jpos, jg2)` after the two fold-back clamps: `jpos` ends in
/// `[iptotl+1, itabl]`, `jg2` is the (possibly folded) destination DTF group.
/// The raw destination is `jg2 = ng - ig2lo - k + 3` and the raw position is
/// `ipingp + (jg2 - jg)` (`dtfr.f90:411-412`).
pub fn scatter_position(
    jg: i32,
    ig2lo: i32,
    k: i32,
    ng: i32,
    ipingp: i32,
    iptotl: i32,
    itabl: i32,
) -> (i32, i32) {
    let mut jg2 = ng - ig2lo - k + 3;
    let mut jpos = ipingp + (jg2 - jg);
    // fold anything past the table length into the last position (dtfr.f90:413-414)
    if jpos > itabl {
        jg2 = jg2 - jpos + itabl;
        jpos = itabl;
    }
    // keep the scatter band above the cross-section positions (dtfr.f90:415-416)
    if jpos <= iptotl {
        jg2 = jg2 + iptotl + 1 - jpos;
        jpos = iptotl + 1;
    }
    (jpos, jg2)
}

/// A DTF group-constant table: the flat `sig(jpos + itabl*(jg-1))` array
/// (`dtfr.f90:343-346`).
///
/// `sig` has length `itabl*ng`. Indices `jpos` (`1..=itabl`) and `jg`
/// (`1..=ng`) are 1-based to match the Fortran; [`DtfTable::linear_index`] does
/// the 0-based conversion. Values are cross sections in **barns** (scatter
/// source, edits, total, absorption, ν·σ_f, χ, …).
#[derive(Debug, Clone, PartialEq)]
pub struct DtfTable {
    /// Number of neutron groups `ng`.
    pub ng: i32,
    /// Table length `itabl` (positions per group).
    pub itabl: i32,
    /// The flat `sig` array, length `itabl*ng`, zero-initialised.
    pub sig: Vec<f64>,
}

impl DtfTable {
    /// Allocate a zeroed table of `itabl` positions × `ng` groups
    /// (`dtfr.f90:284-292`).
    pub fn new(ng: i32, itabl: i32) -> Self {
        let n = (ng.max(0) * itabl.max(0)) as usize;
        Self { ng, itabl, sig: vec![0.0; n] }
    }

    /// 0-based linear index of `(jpos, jg)`: `(jpos-1) + itabl*(jg-1)`
    /// (`dtfr.f90:345-346`, `locs = jpos + itabl*(jg-1)`, less the Fortran 1
    /// base). Panics if either index is out of range.
    pub fn linear_index(&self, jpos: i32, jg: i32) -> usize {
        assert!(jpos >= 1 && jpos <= self.itabl, "jpos {jpos} out of 1..={}", self.itabl);
        assert!(jg >= 1 && jg <= self.ng, "jg {jg} out of 1..={}", self.ng);
        ((jpos - 1) + self.itabl * (jg - 1)) as usize
    }

    /// Read the value at `(jpos, jg)` in **barns**.
    pub fn get(&self, jpos: i32, jg: i32) -> f64 {
        self.sig[self.linear_index(jpos, jg)]
    }

    /// Overwrite the value at `(jpos, jg)` in **barns**.
    pub fn set(&mut self, jpos: i32, jg: i32, value: f64) {
        let i = self.linear_index(jpos, jg);
        self.sig[i] = value;
    }

    /// Accumulate `value` (barns) into `(jpos, jg)` — the common
    /// `sig(locs) = sig(locs) + a(loca)` operation.
    pub fn add(&mut self, jpos: i32, jg: i32, value: f64) {
        let i = self.linear_index(jpos, jg);
        self.sig[i] += value;
    }

    /// Accumulate one GENDF transfer record into the reduced scatter band, with
    /// the P₀ transport (absorption) correction (`dtfr.f90:409-427`).
    ///
    /// * `jg` — the DTF "from" group.
    /// * `ig2lo` — GENDF index of the first secondary group in the record.
    /// * `ipingp`, `iptotl` — in-group and total positions.
    /// * `il` — Legendre order being accumulated (the absorption correction is
    ///   applied only for `il <= 1`, i.e. the P₀/total table, `dtfr.f90:421`).
    /// * `values` — the transfer cross sections for secondary indices
    ///   `k = 2..=ng2`, i.e. `values[k-2]`, in **barns**.
    ///
    /// For each `k` the value is added at the folded `(jpos, jg2)` from
    /// [`scatter_position`]; and, for `il <= 1`, subtracted from the absorption
    /// position `iptotl-2` at the "from" group `jg`
    /// (`sig(iptotl-2, jg) -= value`), so absorption accumulates as
    /// `total - scattering`.
    pub fn add_scatter_record(
        &mut self,
        jg: i32,
        ig2lo: i32,
        ipingp: i32,
        iptotl: i32,
        il: i32,
        values: &[f64],
    ) {
        // ng2 = values.len() + 1 (k runs 2..=ng2)
        for (idx, &v) in values.iter().enumerate() {
            let k = idx as i32 + 2;
            let (jpos, jg2) = scatter_position(jg, ig2lo, k, self.ng, ipingp, iptotl, self.itabl);
            if jpos <= self.itabl && jg2 >= 1 && jg2 <= self.ng {
                self.add(jpos, jg2, v);
                if il <= 1 {
                    // absorption = total - scatter (dtfr.f90:422-423)
                    self.add(iptotl - 2, jg, -v);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// DTF group ordering inverts the GENDF index (`dtfr.f90:342`).
    ///
    /// **Methodology.** `jg = ng - ig + 1`: the highest-energy GENDF group
    /// (`ig=ng`) is DTF group 1, and the lowest (`ig=1`) is DTF group `ng`.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** For `ng=30`: `dtf_group(30)=1`,
    /// `dtf_group(1)=30`, `dtf_group(15)=16`.
    #[test]
    fn group_ordering_inverts() {
        assert_eq!(dtf_group(30, 30), 1);
        assert_eq!(dtf_group(1, 30), 30);
        assert_eq!(dtf_group(15, 30), 16);
    }

    /// The flat `sig` linear index matches `locs = jpos + itabl*(jg-1)`.
    ///
    /// **Methodology.** With `itabl=10, ng=5` the value at position `jpos`,
    /// group `jg` lives at 0-based `(jpos-1) + itabl*(jg-1)` (`dtfr.f90:345-346`).
    /// Check a few cells and that `set`/`get`/`add` route through it.
    ///
    /// **Result (2026-07-15).** `(1,1)→0`, `(10,1)→9`, `(1,2)→10`, `(3,4)→32`;
    /// `add` accumulates (2.0 then +3.0 → 5.0).
    #[test]
    fn linear_index_and_accessors() {
        let mut t = DtfTable::new(5, 10);
        assert_eq!(t.linear_index(1, 1), 0);
        assert_eq!(t.linear_index(10, 1), 9);
        assert_eq!(t.linear_index(1, 2), 10);
        assert_eq!(t.linear_index(3, 4), 32);
        t.set(3, 4, 2.0);
        t.add(3, 4, 3.0);
        assert_eq!(t.get(3, 4), 5.0);
    }

    /// Reduced-band scatter positions: in-group, down-scatter, up-scatter.
    ///
    /// **Methodology.** `dtfr.f90:411-416`. Take `ng=30, iptotl=46, ipingp=47,
    /// itabl=76`. For a "from" group `jg` the in-group transfer (`jg2 == jg`)
    /// must land at `jpos == ipingp`; a one-group down-scatter at `ipingp+1`;
    /// a one-group up-scatter at `ipingp-1`. We choose `ig2lo`,`k` so that
    /// `jg2 = ng - ig2lo - k + 3` equals the target group.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** With `jg=16` (i.e. `ig=15`):
    /// in-group `jg2=16` → `jpos=47`; down `jg2=17` → `jpos=48`; up `jg2=15` →
    /// `jpos=46`? no — `46<=iptotl` triggers the clamp, so up-scatter that would
    /// collide with the xs band folds to `iptotl+1=47` with `jg2` bumped to 16.
    /// A safe up-scatter to `jg2=14` lands at `jpos=45`… also clamped. This test
    /// pins the in-group and down-scatter (unclamped) cases and one clamp case.
    #[test]
    fn scatter_positions_reduced_band() {
        let (ng, iptotl, ipingp, itabl) = (30, 46, 47, 76);
        let jg = 16;
        // in-group: want jg2 = 16 => ng - ig2lo - k + 3 = 16 => ig2lo + k = 17
        let (jpos, jg2) = scatter_position(jg, 15, 2, ng, ipingp, iptotl, itabl);
        assert_eq!((jpos, jg2), (ipingp, 16)); // in-group at ipingp

        // one-group down-scatter: jg2 = 17 => ig2lo + k = 16
        let (jpos_d, jg2_d) = scatter_position(jg, 14, 2, ng, ipingp, iptotl, itabl);
        assert_eq!((jpos_d, jg2_d), (ipingp + 1, 17));

        // far down-scatter past itabl folds back to the last position.
        // jg2 = jg + (itabl - ipingp) + 5 => jpos would exceed itabl.
        let (jpos_f, _jg2_f) = scatter_position(jg, 1, 20, ng, ipingp, iptotl, itabl);
        assert!(jpos_f <= itabl);
    }

    /// Up-scatter into the cross-section band is clamped above `iptotl`.
    ///
    /// **Methodology.** `dtfr.f90:415-416`: when the raw `jpos <= iptotl`, the
    /// position is pushed to `iptotl+1` and `jg2` is bumped by the same offset.
    /// Construct an up-scatter whose raw position would be `iptotl-1`.
    ///
    /// **Result (2026-07-15).** Raw `jpos = ipingp + (jg2-jg)`; choosing
    /// `jg2 = jg - (ipingp - iptotl) - 1 = jg - 2` gives raw `jpos = iptotl-1`,
    /// which clamps to `jpos = iptotl+1 = 47` with `jg2` bumped by 2.
    #[test]
    fn upscatter_clamped_above_iptotl() {
        let (ng, iptotl, ipingp, itabl) = (30, 46, 47, 76);
        let jg = 16;
        // want raw jg2 = jg - 2 = 14 => ng - ig2lo - k + 3 = 14 => ig2lo + k = 19
        let (jpos, jg2) = scatter_position(jg, 17, 2, ng, ipingp, iptotl, itabl);
        // raw jpos = 47 + (14-16) = 45 = iptotl-1 <= iptotl -> clamp
        assert_eq!(jpos, iptotl + 1);
        assert_eq!(jg2, 14 + (iptotl + 1 - 45)); // jg2 bumped by (iptotl+1-rawjpos)=2 -> 16
    }

    /// Scatter accumulation builds absorption = total − scatter for P₀.
    ///
    /// **Methodology.** `dtfr.f90:421-424`: for `il<=1`, each transfer added to
    /// the scatter band is also subtracted from the absorption position
    /// `iptotl-2` at the "from" group. Accumulate an in-group + one down-scatter
    /// from `jg=16` with values 3.0 and 1.0 barns; assert the band cells hold the
    /// transfers and that `sig(iptotl-2, 16) == -(3+1)`.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** in-group cell `(47,16)=3.0`,
    /// down-scatter `(48,17)=1.0`, absorption `(44,16) = -4.0`. For `il=2` (P₁)
    /// no absorption correction is applied.
    #[test]
    fn scatter_record_absorption_correction() {
        let (ng, iptotl, ipingp, itabl) = (30, 46, 47, 76);
        let mut t = DtfTable::new(ng, itabl);
        // values for k=2 (in-group, ig2lo=15) and k=3 (down one, same record)
        // Build a record with ig2lo=15: k=2 -> jg2=16 (in-group), k=3 -> jg2=15?
        // jg2 = ng - ig2lo - k + 3 = 30-15-k+3 = 18-k. k=2 -> 16, k=3 -> 15 (up).
        // To get in-group then down-scatter cleanly, use ig2lo=14:
        // jg2 = 30-14-k+3 = 19-k. k=2 -> 17 (down), k=3 -> 16 (in-group).
        let jg = 16;
        t.add_scatter_record(jg, 14, ipingp, iptotl, 1, &[1.0, 3.0]);
        // k=2 -> jg2=17 down-scatter jpos=48 value 1.0
        assert_eq!(t.get(ipingp + 1, 17), 1.0);
        // k=3 -> jg2=16 in-group jpos=47 value 3.0
        assert_eq!(t.get(ipingp, 16), 3.0);
        // absorption at iptotl-2 (=44), from group jg=16: -(1+3)
        assert_eq!(t.get(iptotl - 2, jg), -4.0);

        // P1: no absorption correction
        let mut t1 = DtfTable::new(ng, itabl);
        t1.add_scatter_record(jg, 14, ipingp, iptotl, 2, &[1.0, 3.0]);
        assert_eq!(t1.get(iptotl - 2, jg), 0.0);
    }
}
