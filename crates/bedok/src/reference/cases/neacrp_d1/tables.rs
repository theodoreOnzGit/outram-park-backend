//! NEACRP-L-335 BWR two-group cross-section tables and their feedback
//! derivatives.
//!
//! # Provenance
//!
//! | | |
//! |---|---|
//! | Original author | Than Yan Ren, Singapore Nuclear Research and Safety Institute (SNRSI) |
//! | Source file | the `sigmavalues` blocks of `neacrpd1.m` |
//! | Snapshot | `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…` |
//! | Permission | translation and open-source publication approved by the author and the project lead; see `docs/bedok-port-scoping.md` §6 |
//! | Benchmark | NEACRP 3-D LWR Core Transient Benchmark, NEA/NSC/DOC(93)25 (NEACRP-L-335 Rev. 1), 1991 |
//!
//! # Nineteen materials, unlabelled
//!
//! Unlike the PWR case, `neacrpd1.m` gives **no comment block naming the
//! materials**. What can be read off the data and the axial column table
//! (`NEACRPD1_COL.csv`): material 1 is the bottom reflector plane and
//! material 4 the top, neither of which fissions; material 19 is the radial
//! reflector (column 10 at every axial level), also non-fissioning; the
//! remaining sixteen are fuelled compositions varying with void history and
//! burnable-absorber loading. Recorded as an observation, not an authority —
//! the reference does not say.
//!
//! # What this case does *not* have
//!
//! - **No `sigmavalues.fp`.** The array is allocated and every assignment to
//!   it is commented out, so `kappa*Sigma_f` is identically zero.
//!   `neacrpd1t.m` rebuilds it from `nu*Sigma_f` because the transient power
//!   normalisation divides by it; the steady solver never reads it.
//! - **No boron feedback**, even though `neacrpd1.m` sets
//!   `params.boron = 1000`.
//! - **No coolant-*temperature* feedback** — only fuel temperature and coolant
//!   density. In a BWR the density (void) feedback dominates, so this is a
//!   defensible modelling choice rather than plainly an omission, but it is
//!   an asymmetry with the PWR case worth knowing about.
//! - **No control-rod cross sections.** The section header is present and the
//!   body is empty.
//!
//! All four are recorded, not repaired — see `docs/bedok-port-scoping.md`
//! §1.0.
//!
//! # Units
//!
//! Cross sections \[1/cm\]. Derivative tables carry those per K (fuel
//! temperature) or per g/cm³ (coolant density).

use crate::error::Result;
use crate::reference::cases::sigmas::SigmaSet;

/// Number of materials in the NEACRP BWR tables.
pub const MATERIALS: usize = 19;
/// Number of energy groups.
pub const GROUPS: usize = 2;

/// Assemble one `SigmaSet` from the per-group tables and the down-scatter
/// column, closing the scattering diagonal as the MATLAB does.
///
/// `kappa_fission` is always zero here: `neacrpd1.m` comments out every
/// assignment to `sigmavalues.fp` and to the `fp` block of each feedback
/// table. The field is kept present-and-zero rather than absent, matching the
/// MATLAB, which does allocate the array.
///
/// # Errors
///
/// Propagates [`SigmaSet::close_self_scatter`]'s error, which cannot fire here
/// because absorption is always supplied.
fn assemble(
    total: [[f64; GROUPS]; MATERIALS],
    nu_fission: [[f64; GROUPS]; MATERIALS],
    absorption: [[f64; GROUPS]; MATERIALS],
    down_scatter: [f64; MATERIALS],
) -> Result<SigmaSet> {
    let mut set = SigmaSet::zeros(MATERIALS, GROUPS);
    for m in 0..MATERIALS {
        set.total[m] = total[m].to_vec();
        set.nu_fission[m] = nu_fission[m].to_vec();
        set.absorption[m] = absorption[m].to_vec();
        set.scatter
            .set_block_2x2(m, [[0.0, 0.0], [down_scatter[m], 0.0]]);
    }
    set.close_self_scatter()?;
    Ok(set)
}

/// Base cross sections at the reference state.
///
/// MATLAB `sigmavalues.tot` / `.f` / `.a` / `.s` of `neacrpd1.m`.
///
/// # Errors
///
/// Cannot fail in practice: the internal assembly step only errors when the
/// absorption table is missing, and it is always supplied here.
pub fn base_sigmas() -> Result<SigmaSet> {
    assemble(
        [
            [0.111030, 0.830012],
            [0.189784, 0.694136],
            [0.188544, 0.693963],
            [0.13427, 0.73005],
            [0.189186, 0.693475],
            [0.188264, 0.693345],
            [0.199654, 0.718647],
            [0.198692, 0.719003],
            [0.189151, 0.693476],
            [0.188381, 0.693427],
            [0.18796, 0.69355],
            [0.188575, 0.693636],
            [0.189091, 0.693478],
            [0.188616, 0.693591],
            [0.187354, 0.69396],
            [0.199871, 0.722345],
            [0.19841, 0.718597],
            [0.197215, 0.719295],
            [0.184542, 1.36864],
        ],
        [
            [0.0, 0.0],
            [0.446986E-02, 0.828220E-01],
            [0.446539E-02, 0.804386E-01],
            [0.0, 0.0],
            [0.413061E-02, 0.738611E-01],
            [0.412726E-02, 0.720736E-01],
            [0.416239E-02, 0.649081E-01],
            [0.416026E-02, 0.636570E-01],
            [0.413003E-02, 0.738046E-01],
            [0.412816E-02, 0.723935E-01],
            [0.412665E-02, 0.716701E-01],
            [0.412888E-02, 0.728618E-01],
            [0.412887E-02, 0.736916E-01],
            [0.412995E-02, 0.730336E-01],
            [0.412543E-02, 0.708631E-01],
            [0.415877E-02, 0.650624E-01],
            [0.416892E-02, 0.643471E-01],
            [0.416619E-02, 0.626218E-01],
            [0.0, 0.0],
        ],
        [
            [3.92E-04, 1.4801E-02],
            [1.02352E-02, 7.49127E-02],
            [1.03417E-02, 7.68592E-02],
            [5.53E-04, 6.22329E-03],
            [1.01071E-02, 6.83185E-02],
            [1.01869E-02, 6.97783E-02],
            [7.09736E-03, 4.84724E-02],
            [7.15434E-03, 4.90908E-02],
            [1.01112E-02, 6.83829E-02],
            [1.01774E-02, 6.96467E-02],
            [1.02185E-02, 7.04713E-02],
            [1.01653E-02, 6.94981E-02],
            [1.01196E-02, 6.85118E-02],
            [1.01582E-02, 6.93833E-02],
            [1.02817E-02, 7.18573E-02],
            [7.08533E-03, 4.82128E-02],
            [7.17808E-03, 5.00243E-02],
            [7.25399E-03, 5.09656E-02],
            [3.59E-04, 1.0868E-02],
        ],
        [
            0.022595, 0.0141764, 0.0142295, 0.018177, 0.0143548, 0.0143946, 0.0164565, 0.0165185,
            0.0143552, 0.0143893, 0.0144036, 0.0143771, 0.0143562, 0.0143789, 0.0144216, 0.016448,
            0.016521, 0.0165892, 0.037579,
        ],
    )
}

/// Fuel-temperature (Doppler) derivatives \[per K\], referenced to 573.15 K.
///
/// MATLAB `sigmavalues.fueltemp` of `neacrpd1.m`.
///
/// Every **group-1 total** derivative is zero while the group-1 *absorption*
/// derivative is positive: the Doppler broadening shows up as resonance
/// capture that is exactly offset in the total by the closing identity, so the
/// within-group scattering diagonal absorbs it. That is a property of the
/// data as supplied, not of the port.
///
/// # Errors
///
/// Cannot fail in practice: the internal assembly step only errors when the
/// absorption table is missing, and it is always supplied here.
pub fn fuel_temperature_derivatives() -> Result<SigmaSet> {
    assemble(
        [
            [0.0, 0.0],
            [0.0, -8.23459E-05],
            [0.0, -8.2524E-05],
            [0.0, 0.0],
            [0.0, -8.24488E-05],
            [0.0, -8.25824E-05],
            [0.0, -8.67114E-05],
            [0.0, -8.68472E-05],
            [0.0, -8.4204E-05],
            [0.0, -8.26201E-05],
            [0.0, -8.26179E-05],
            [0.0, -8.25289E-05],
            [0.0, -8.23634E-05],
            [0.0, -8.26954E-05],
            [0.0, -8.26889E-05],
            [0.0, -8.66561E-05],
            [0.0, -8.67723E-05],
            [0.0, -8.70674E-05],
            [0.0, 0.0],
        ],
        [
            [0.0, 0.0],
            [0.0, -0.350770E-04],
            [0.0, -0.341409E-04],
            [0.0, 0.0],
            [0.0, -0.313155E-04],
            [0.0, -0.306135E-04],
            [0.0, -0.277381E-04],
            [0.0, -0.272325E-04],
            [0.0, -0.312795E-04],
            [0.0, -0.307625E-04],
            [0.0, -0.304546E-04],
            [0.0, -0.309227E-04],
            [0.0, -0.312074E-04],
            [0.0, -0.310605E-04],
            [0.0, -0.301369E-04],
            [0.0, -0.277728E-04],
            [0.0, -0.277559E-04],
            [0.0, -0.268636E-04],
            [0.0, 0.0],
        ],
        [
            [0.0, 0.0],
            [0.200902E-04, -0.262873E-04],
            [0.201801E-04, -0.270360E-04],
            [0.0, 0.0],
            [0.204046E-04, -0.239924E-04],
            [0.204720E-04, -0.245539E-04],
            [0.231603E-04, -0.172597E-04],
            [0.232572E-04, -0.174963E-04],
            [0.203980E-04, -0.240056E-04],
            [0.204749E-04, -0.245168E-04],
            [0.204833E-04, -0.248087E-04],
            [0.204384E-04, -0.244344E-04],
            [0.203849E-04, -0.240320E-04],
            [0.204806E-04, -0.244425E-04],
            [0.205059E-04, -0.253184E-04],
            [0.231300E-04, -0.171504E-04],
            [0.232614E-04, -0.178265E-04],
            [0.234010E-04, -0.182100E-04],
            [0.0, 0.0],
        ],
        [
            0.0,
            -0.160580E-04,
            -0.161574E-04,
            0.0,
            -0.162970E-04,
            -0.163716E-04,
            -0.189002E-04,
            -0.189895E-04,
            -0.162923E-04,
            -0.163709E-04,
            -0.163872E-04,
            -0.163375E-04,
            -0.162829E-04,
            -0.163695E-04,
            -0.164185E-04,
            -0.188723E-04,
            -0.189906E-04,
            -0.191256E-04,
            0.0,
        ],
    )
}

/// Coolant-density derivatives \[per g/cm³\], referenced to 0.55 g/cm³.
///
/// MATLAB `sigmavalues.coolden` of `neacrpd1.m`. This is the dominant
/// feedback of a BWR: it is what couples the void distribution back into the
/// neutronics.
///
/// # Errors
///
/// Cannot fail in practice: the internal assembly step only errors when the
/// absorption table is missing, and it is always supplied here.
pub fn coolant_density_derivatives() -> Result<SigmaSet> {
    assemble(
        [
            [2.1932E-07, 7.01285E-06],
            [0.130164, 0.759141],
            [0.12875, 0.752907],
            [1.75456E-06, 1.40365E-05],
            [0.129802, 0.77124],
            [0.128744, 0.766599],
            [0.127443, 0.750235],
            [0.126783, 0.746987],
            [0.129572, 0.771049],
            [0.128909, 0.766905],
            [0.128450, 0.762266],
            [0.129158, 0.765378],
            [0.12965, 0.770663],
            [0.129248, 0.767578],
            [0.127871, 0.753668],
            [0.127402, 0.73863],
            [0.126972, 0.745357],
            [0.126127, 0.741955],
            [0.0, 1.75456E-06],
        ],
        [
            [0.0, 0.0],
            [0.111084E-02, 0.246360E-01],
            [0.111093E-02, 0.236482E-01],
            [0.0, 0.0],
            [0.960766E-03, 0.211180E-01],
            [0.960828E-03, 0.203775E-01],
            [0.696670E-03, 0.130177E-01],
            [0.693127E-03, 0.124660E-01],
            [0.959197E-03, 0.211062E-01],
            [0.962980E-03, 0.205383E-01],
            [0.963165E-03, 0.204017E-01],
            [0.963117E-03, 0.208959E-01],
            [0.956030E-03, 0.210825E-01],
            [0.967291E-03, 0.208614E-01],
            [0.967730E-03, 0.204502E-01],
            [0.692305E-03, 0.127594E-01],
            [0.704641E-03, 0.137254E-01],
            [0.701297E-03, 0.126783E-01],
            [0.0, 0.0],
        ],
        [
            [0.856719E-09, 0.109660E-06],
            [0.238730E-02, 0.126336E-01],
            [0.247360E-02, 0.127195E-01],
            [0.0, -0.685375E-08],
            [0.227347E-02, 0.108673E-01],
            [0.233817E-02, 0.109320E-01],
            [0.170083E-02, 0.126452E-01],
            [0.171473E-02, 0.125016E-01],
            [0.228093E-02, 0.108792E-01],
            [0.232403E-02, 0.109351E-01],
            [0.236219E-02, 0.110489E-01],
            [0.231898E-02, 0.110057E-01],
            [0.229564E-02, 0.109047E-01],
            [0.229551E-02, 0.109412E-01],
            [0.241000E-02, 0.112829E-01],
            [0.169762E-02, 0.127121E-01],
            [0.172207E-02, 0.126051E-01],
            [0.173860E-02, 0.125542E-01],
            [0.428360E-09, 0.109660E-06],
        ],
        [
            0.219320E-06,
            0.196627E-01,
            0.197654E-01,
            0.219320E-06,
            0.198449E-01,
            0.199218E-01,
            0.197129E-01,
            0.197951E-01,
            0.198434E-01,
            0.199168E-01,
            0.199429E-01,
            0.198915E-01,
            0.198407E-01,
            0.199065E-01,
            0.199848E-01,
            0.197119E-01,
            0.197836E-01,
            0.198932E-01,
            0.109660E-06,
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_tables_have_nineteen_materials() {
        let s = base_sigmas().expect("assembles");
        assert_eq!(s.materials(), 19);
        assert_eq!(s.ngroups(), 2);
        assert_eq!(s.total[0][0], 0.111030);
        assert_eq!(s.total[18][1], 1.36864);
    }

    /// Materials 1, 4 and 19 — the two axial reflector planes and the radial
    /// reflector — do not fission; the other sixteen do.
    #[test]
    fn three_materials_are_reflectors() {
        let s = base_sigmas().expect("assembles");
        for m in [0usize, 3, 18] {
            assert_eq!(
                s.nu_fission[m].iter().sum::<f64>(),
                0.0,
                "material {} should not fission",
                m + 1
            );
        }
        for m in [1usize, 2, 4, 17] {
            assert!(s.nu_fission[m][1] > 0.0, "material {}", m + 1);
        }
    }

    /// `sigmavalues.fp` is commented out in the source, so every
    /// kappa-fission entry is zero — the defect `neacrpd1t.m` has to work
    /// around.
    #[test]
    fn kappa_fission_is_identically_zero() {
        let s = base_sigmas().expect("assembles");
        assert_eq!(s.kappa_fission.len(), MATERIALS);
        assert!(s
            .kappa_fission
            .iter()
            .all(|row| row.iter().all(|v| *v == 0.0)));
    }

    #[test]
    fn scattering_closes_the_removal_balance() {
        let s = base_sigmas().expect("assembles");
        for m in 0..MATERIALS {
            for g in 0..GROUPS {
                let out: f64 = (0..GROUPS)
                    .filter(|gt| *gt != g)
                    .map(|gt| s.scatter.get(m, gt, g))
                    .sum();
                let balance = s.scatter.get(m, g, g) + out + s.absorption[m][g];
                assert!(
                    (balance - s.total[m][g]).abs() < 1e-14,
                    "material {}",
                    m + 1
                );
            }
        }
    }

    /// The BWR density feedback is large and positive on fission: losing
    /// moderator density loses reactivity, which is the void feedback the case
    /// exists to exercise.
    #[test]
    fn density_feedback_dominates_and_has_the_right_sign() {
        let density = coolant_density_derivatives().expect("assembles");
        assert!(density.nu_fission[1][1] > 0.0);
        assert!(
            density.total[1][1] > 0.5,
            "density derivative is O(1) per g/cm3"
        );

        let doppler = fuel_temperature_derivatives().expect("assembles");
        assert!(
            doppler.absorption[1][0] > 0.0,
            "Doppler raises fast capture"
        );
        assert!(doppler.nu_fission[1][1] < 0.0);
        // Every group-1 total derivative is zero, as supplied.
        assert!(doppler.total.iter().all(|row| row[0] == 0.0));
    }
}
