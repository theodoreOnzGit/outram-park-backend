//! Per-group cross-section pieces (excluding the overall `4*pi/E` factor,
//! applied once across all groups by [`super::crosss`]) — ported from
//! `sectio`'s non-angular core (`samm.f90:6286-6367`).
//!
//! **Not ported here (deferred):** the `Want_Angular_Dist`-gated block
//! (`samm.f90:6369-6437`, needing `gcphase`/`cscs`) — no caller until
//! angular distributions are built alongside `ERRORR`, per this crate's
//! `README.md` scope-history note.
//!
//! # Output indexing — a genuine Fortran quirk, ported as-is
//!
//! `crss` is indexed by **particle-pair number** for reaction channels
//! (`ip=3..npp`, using the raw ENDF pair index directly) but the first two
//! slots are hardcoded regardless of pair numbering: `crss[0]` always
//! holds the **elastic** contribution (physically the entrance pair,
//! conventionally pair 2) and `crss[1]` always holds the **capture/
//! absorption** contribution (physically the eliminated pair, pair 1) —
//! i.e. positions 1 and 2 are swapped relative to their own pair numbers.
//! This is upstream's own convention (`samm.f90:6302-6345`'s comments:
//! "Entrance channel, ipp=2 ... crss(1)"; "Ipp=1 term, sort-of ...
//! crss(2)"), not a translation artifact — ported literally.

use crate::samm::context::GroupQuantumInfo;

/// Compute this group's contribution to `crss[pair_index]` (0-indexed by
/// particle-pair number minus one, **except** `crss[0]`=elastic and
/// `crss[1]`=capture — see this module's doc comment) — ported from
/// `sectio`'s non-angular core (`samm.f90:6286-6367`).
///
/// `zke`: per-channel `zke` scale factor ([`crate::samm::context::ChannelKinematics::zke`]).
/// `sinsqr`/`sin2ph`/`xxxxr`/`xxxxi`: [`super::setr::setr`]/[`super::assembly::setxqx`]'s outputs.
/// `particle_pair`: 1-indexed particle-pair number per channel (same order
/// as `zke` etc.), i.e. [`crate::samm::mf2::RmlChannel::particle_pair`].
pub fn sectio(
    npp: usize,
    info: &GroupQuantumInfo,
    zke: &[f64],
    particle_pair: &[usize],
    sinsqr: &[f64],
    sin2ph: &[f64],
    xxxxr: &[f64],
    xxxxi: &[f64],
) -> Vec<f64> {
    let nent = info.n_entrance;
    let next = info.n_exit;
    let nchan = zke.len();

    let mut crss = vec![0.0_f64; npp];

    #[inline]
    fn packed(i: usize, j: usize) -> usize {
        // 1-indexed i<=j -> 0-indexed flat packed position.
        if i <= j {
            i - 1 + j * (j - 1) / 2
        } else {
            j - 1 + i * (i - 1) / 2
        }
    }

    // samm.f90:6302-6324 -- elastic, crss[0].
    let mut ii = 0usize;
    let mut termn_total = 0.0_f64;
    for i in 1..=nent {
        let zz = zke[i - 1].powi(2);
        ii += i;
        let mut termn = sinsqr[i - 1] * (1.0 - 2.0 * xxxxi[ii - 1]) - sin2ph[i - 1] * xxxxr[ii - 1];
        termn /= zz;
        for j in 1..=i {
            let ij = packed(j, i);
            let mut ar = (xxxxr[ij].powi(2) + xxxxi[ij].powi(2)) / zz;
            if i != j {
                ar += ar;
            }
            termn += ar;
        }
        termn_total += termn;
    }
    crss[0] = termn_total * info.goj;

    // samm.f90:6326-6345 -- capture/absorption, crss[1].
    let mut ii = 0usize;
    let mut terma_total = 0.0_f64;
    for i in 1..=nent {
        ii += i;
        let zz = zke[i - 1].powi(2);
        let mut terma = xxxxi[ii - 1] / zz;
        for j in 1..=i {
            let ij = packed(j, i);
            let mut ar = (-xxxxr[ij].powi(2) - xxxxi[ij].powi(2)) / zz;
            if i != j {
                ar += ar;
            }
            terma += ar;
        }
        terma_total += terma;
    }
    crss[1] = terma_total * info.goj;

    // samm.f90:6347-6367 -- all other (explicit reaction) channels,
    // classed by their own particle-pair number.
    for jj in 1..=next {
        let j = jj + nent;
        if j <= nchan {
            let ip = particle_pair[j - 1];
            for i in 1..=nent {
                let zz = zke[i - 1].powi(2);
                let ij = (j * (j - 1)) / 2 + i - 1; // ijkl(i,j), i<j always here
                crss[ip - 1] += (xxxxr[ij].powi(2) + xxxxi[ij].powi(2)) / zz;
            }
        }
    }
    if npp > 2 {
        for ip in 3..=npp {
            crss[ip - 1] *= info.goj;
        }
    }

    crss
}
