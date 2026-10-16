//! Thinning a reconstructed cross section for a phone, shared by the
//! neutronics lesson examples (`lesson_total_xs`, `lesson_reactions`,
//! `lesson_slowing_down`; included with `#[path]`, so it is not an example of
//! its own). The energy axis [`E_LO`]..[`E_HI`] is cut into [`BINS`] equal
//! steps in log E, and in each step the point with the highest and the point
//! with the lowest σ are kept, in energy order. Every peak and trough
//! survives to the plot's resolution; nothing is smoothed or averaged.
//! Non-positive σ is dropped (it cannot be drawn on a log axis).

/// Steps in log E across 1e-5 eV .. 2e7 eV.
pub const BINS: usize = 900;
pub const E_LO: f64 = 1.0e-5;
pub const E_HI: f64 = 2.0e7;

/// Keep the highest and lowest point of each log-E step, in energy order.
pub fn thin(pairs: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let (l0, l1) = (E_LO.log10(), E_HI.log10());
    let mut best: Vec<Option<(usize, usize)>> = vec![None; BINS];
    for (i, &(e, s)) in pairs.iter().enumerate() {
        if !(E_LO..=E_HI).contains(&e) || s <= 0.0 {
            continue;
        }
        let b = (((e.log10() - l0) / (l1 - l0)) * BINS as f64).floor().clamp(0.0, (BINS - 1) as f64) as usize;
        best[b] = Some(match best[b] {
            None => (i, i),
            Some((lo, hi)) => (if s < pairs[lo].1 { i } else { lo }, if s > pairs[hi].1 { i } else { hi }),
        });
    }
    let mut keep: Vec<usize> = best.into_iter().flatten().flat_map(|(a, b)| [a, b]).collect();
    keep.sort_unstable();
    keep.dedup();
    keep.into_iter().map(|i| pairs[i]).collect()
}
