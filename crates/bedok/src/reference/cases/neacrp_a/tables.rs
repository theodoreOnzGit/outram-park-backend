//! NEACRP-L-335 PWR two-group cross-section tables and their feedback
//! derivatives.
//!
//! # Provenance
//!
//! | | |
//! |---|---|
//! | Original author | Than Yan Ren, Singapore Nuclear Research and Safety Institute (SNRSI) |
//! | Source file | the `sigmavalues` blocks of `neacrpa2.m` (reproduced verbatim in `neacrpa2t.m` and `neacrpa1t.m`) |
//! | Snapshot | `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…` |
//! | Permission | translation and open-source publication approved by the author and the project lead; see `docs/bedok-port-scoping.md` §6 |
//! | Benchmark | NEACRP 3-D LWR Core Transient Benchmark, NEA/NSC/DOC(93)25 (NEACRP-L-335 Rev. 1), 1991 — public, citable, clean under `DATA_POLICY.md` |
//!
//! # Layout
//!
//! Each table is an 11-row array of `[group 1, group 2]` values, in the same
//! order the MATLAB assigns them in its `(1:6,:)` and `(7:11,:)` slabs, so a
//! reader can diff a row against the source line. The scattering matrix is
//! given by its down-scatter column alone; the within-group diagonal is then
//! closed on `total - absorption - out-scatter`, exactly as the MATLAB's
//! trailing two lines do.
//!
//! # Materials
//!
//! | Index | Composition |
//! |---|---|
//! | 1 | Axial reflector |
//! | 2 | Radial reflector |
//! | 3 | Radial reflector, re-entrant corner |
//! | 4 | 2.1 w/o fuel |
//! | 5 | 2.6 w/o fuel |
//! | 6 | 3.1 w/o fuel |
//! | 7 | 2.6 w/o + 12 burnable absorber rods |
//! | 8 | 2.6 w/o + 16 burnable absorber rods |
//! | 9 | 2.6 w/o + 20 burnable absorber rods |
//! | 10 | 3.1 w/o + 12 burnable absorber rods |
//! | 11 | 3.1 w/o + 16 burnable absorber rods |
//!
//! # Units
//!
//! Cross sections \[1/cm\]; `kappa_fission` \[J/cm\]. A derivative table
//! carries those divided by ppm, K or g/cm³ as its variable requires.

use crate::error::Result;
use crate::reference::cases::sigmas::SigmaSet;

/// Number of materials in the NEACRP PWR tables.
pub const MATERIALS: usize = 11;
/// Number of energy groups.
pub const GROUPS: usize = 2;

/// Assemble one `SigmaSet` from the four per-group tables and the
/// down-scatter column, closing the scattering diagonal as the MATLAB does.
///
/// # Errors
///
/// Propagates [`SigmaSet::close_self_scatter`]'s error, which cannot fire here
/// because absorption is always supplied.
fn assemble(
    total: [[f64; GROUPS]; MATERIALS],
    nu_fission: [[f64; GROUPS]; MATERIALS],
    kappa_fission: [[f64; GROUPS]; MATERIALS],
    absorption: [[f64; GROUPS]; MATERIALS],
    down_scatter: [f64; MATERIALS],
) -> Result<SigmaSet> {
    let mut set = SigmaSet::zeros(MATERIALS, GROUPS);
    for m in 0..MATERIALS {
        set.total[m] = total[m].to_vec();
        set.nu_fission[m] = nu_fission[m].to_vec();
        set.kappa_fission[m] = kappa_fission[m].to_vec();
        set.absorption[m] = absorption[m].to_vec();
        // sigmavalues.s(m,:,:) = [0 0; down 0]; the diagonal is filled next.
        set.scatter
            .set_block_2x2(m, [[0.0, 0.0], [down_scatter[m], 0.0]]);
    }
    set.close_self_scatter()?;
    Ok(set)
}

/// Base cross sections at the reference state.
///
/// MATLAB `sigmavalues.tot` / `.f` / `.fp` / `.a` / `.s` of `neacrpa2.m`.
///
/// # Errors
///
/// Cannot fail in practice: the internal assembly step only errors when the
/// absorption table is missing, and it is always supplied here.
pub fn base_sigmas() -> Result<SigmaSet> {
    assemble(
        [
            [0.0532058, 0.386406],
            [0.295609, 2.45931],
            [0.295609, 2.45931],
            [0.222117, 0.803140],
            [0.221914, 0.795538],
            [0.221715, 0.789253],
            [0.222039, 0.776230],
            [0.222083, 0.769969],
            [0.222127, 0.763813],
            [0.221836, 0.770705],
            [0.221878, 0.764704],
        ],
        [
            [0.0, 0.0],
            [0.0, 0.0],
            [0.0, 0.0],
            [4.98277E-03, 8.39026E-02],
            [5.57659E-03, 9.98629E-02],
            [6.15047E-03, 1.14667E-01],
            [5.55010E-03, 9.85576E-02],
            [5.54083E-03, 9.80059E-02],
            [5.53137E-03, 9.74109E-02],
            [6.12382E-03, 1.13241E-01],
            [6.11444E-03, 1.12635E-01],
        ],
        [
            [0.0, 0.0],
            [0.0, 0.0],
            [0.0, 0.0],
            [6.11190E-14, 1.10152E-12],
            [6.89181E-14, 1.31106E-12],
            [7.64603E-14, 1.50541E-12],
            [6.86391E-14, 1.29393E-12],
            [6.85391E-14, 1.28669E-12],
            [6.84379E-14, 1.27888E-12],
            [7.61794E-14, 1.48670E-12],
            [7.60778E-14, 1.47876E-12],
        ],
        [
            [3.73279E-04, 1.77215E-02],
            [1.18782E-03, 2.52618E-01],
            [1.18782E-03, 2.52618E-01],
            [8.71774E-03, 6.52550E-02],
            [9.06133E-03, 7.23354E-02],
            [9.38496E-03, 7.89203E-02],
            [9.31692E-03, 7.96328E-02],
            [9.40032E-03, 8.21087E-02],
            [9.48286E-03, 8.45912E-02],
            [9.63720E-03, 8.61187E-02],
            [9.71937E-03, 8.85488E-02],
        ],
        [
            0.0264554, 0.0231613, 0.0200808, 0.0182498, 0.0180040, 0.0177670, 0.0171381, 0.0168501,
            0.0165626, 0.0169043, 0.0166175,
        ],
    )
}

/// Boron-concentration derivatives \[per ppm\], referenced to 1200.2 ppm.
///
/// MATLAB `sigmavalues.boron`.
///
/// # A variant left in the source
///
/// `neacrpa2.m` carries a commented-out alternative for
/// `sigmavalues.boron.tot(1:6,:)` that zeroes the group-2 entry of the two
/// reflector materials (`0 7.76184E-04` → `0 0`). The **active** line, with
/// the reflector entries present, is the one ported. Recorded because a
/// disagreement with the benchmark's boron worth would make that switch the
/// first thing to try.
///
/// # Errors
///
/// Cannot fail in practice: the internal assembly step only errors when the
/// absorption table is missing, and it is always supplied here.
pub fn boron_derivatives() -> Result<SigmaSet> {
    assemble(
        [
            [6.11833E-08, 5.17535E-06],
            [0.0, 7.76184E-04],
            [0.0, 7.76184E-04],
            [3.47809E-08, -9.76510E-06],
            [3.53826E-08, -8.50169E-06],
            [3.59838E-08, -7.46251E-06],
            [3.37806E-08, -6.73744E-06],
            [3.32495E-08, -6.19725E-06],
            [3.27201E-08, -5.68220E-06],
            [3.43859E-08, -5.86898E-06],
            [3.38559E-08, -5.38345E-06],
        ],
        [
            [0.0, 0.0],
            [0.0, 0.0],
            [0.0, 0.0],
            [-1.12099E-09, -2.43045E-06],
            [-1.67880E-09, -2.72445E-06],
            [-2.21038E-09, -2.95883E-06],
            [-1.71323E-09, -2.55359E-06],
            [-1.72421E-09, -2.48880E-06],
            [-1.73502E-09, -2.42240E-06],
            [-2.24335E-09, -2.77657E-06],
            [-2.25369E-09, -2.70780E-06],
        ],
        [
            [0.0, 0.0],
            [0.0, 0.0],
            [0.0, 0.0],
            [-1.76188E-20, -3.19085E-17],
            [-2.49965E-20, -3.57680E-17],
            [-3.20225E-20, -3.88451E-17],
            [-2.49965E-20, -3.35223E-17],
            [-2.54896E-20, -3.26704E-17],
            [-2.56049E-20, -3.17976E-17],
            [-3.20225E-20, -3.64509E-17],
            [-3.24873E-20, -3.55476E-17],
        ],
        [
            [1.87731E-07, 1.02635E-05],
            [0.0, 8.44695E-05],
            [0.0, 8.44695E-05],
            [1.28505E-07, 7.08807E-06],
            [1.26709E-07, 6.82311E-06],
            [1.24986E-07, 6.59798E-06],
            [1.19869E-07, 6.29310E-06],
            [1.17585E-07, 6.11904E-06],
            [1.15319E-07, 5.94711E-06],
            [1.18186E-07, 6.08443E-06],
            [1.15917E-07, 5.91697E-06],
        ],
        [
            7.91457E-10,
            0.0,
            0.0,
            -1.08590E-07,
            -1.06951E-07,
            -1.05374E-07,
            -1.00873E-07,
            -9.88578E-08,
            -9.68489E-08,
            -9.93312E-08,
            -9.73291E-08,
        ],
    )
}

/// Fuel-temperature (Doppler) derivatives \[per K\], referenced to 891.45 K.
///
/// MATLAB `sigmavalues.fueltemp`. The three reflector materials have no fuel,
/// so their derivatives are zero.
///
/// # Errors
///
/// Cannot fail in practice: the internal assembly step only errors when the
/// absorption table is missing, and it is always supplied here.
pub fn fuel_temperature_derivatives() -> Result<SigmaSet> {
    assemble(
        [
            [0.0, 0.0],
            [0.0, 0.0],
            [0.0, 0.0],
            [-3.09197E-05, -0.000137292],
            [-3.08607E-05, -0.000117481],
            [-3.09165E-05, -0.000101337],
            [-3.13746E-05, -0.000108271],
            [-3.15503E-05, -0.000105521],
            [-3.17281E-05, -0.000102525],
            [-3.14192E-05, -9.38886E-05],
            [-3.15908E-05, -9.17126E-05],
        ],
        [
            [0.0, 0.0],
            [0.0, 0.0],
            [0.0, 0.0],
            [6.40134E-07, -5.63037E-05],
            [9.97431E-07, -6.04155E-05],
            [1.41847E-06, -0.000063096],
            [9.45431E-07, -5.79662E-05],
            [9.26078E-07, -5.71108E-05],
            [9.05802E-07, -5.61543E-05],
            [1.35642E-06, -6.05052E-05],
            [1.33336E-06, -5.96284E-05],
        ],
        [
            [0.0, 0.0],
            [0.0, 0.0],
            [0.0, 0.0],
            [7.15412E-18, -7.39188E-16],
            [1.18685E-17, -7.93170E-16],
            [1.74269E-17, -8.28363E-16],
            [1.18685E-17, -7.60849E-16],
            [1.08935E-17, -7.49575E-16],
            [1.06166E-17, -7.36969E-16],
            [1.74269E-17, -7.94252E-16],
            [1.62769E-17, -7.82716E-16],
        ],
        [
            [0.0, 0.0],
            [0.0, 0.0],
            [0.0, 0.0],
            [3.49709E-05, -3.71806E-05],
            [3.51798E-05, -3.77039E-05],
            [3.53841E-05, -3.77558E-05],
            [3.48699E-05, -3.72748E-05],
            [3.47274E-05, -3.71808E-05],
            [3.46026E-05, -3.70201E-05],
            [3.50637E-05, -3.71403E-05],
            [3.49119E-05, -3.69909E-05],
        ],
        [
            0.0,
            0.0,
            0.0,
            -2.75536E-05,
            -2.76766E-05,
            -2.78390E-05,
            -2.73550E-05,
            -2.72381E-05,
            -2.71169E-05,
            -2.75049E-05,
            -2.73835E-05,
        ],
    )
}

/// Coolant-temperature derivatives \[per K\], referenced to 579.75 K.
///
/// MATLAB `sigmavalues.cooltemp`.
///
/// # Errors
///
/// Cannot fail in practice: the internal assembly step only errors when the
/// absorption table is missing, and it is always supplied here.
pub fn coolant_temperature_derivatives() -> Result<SigmaSet> {
    assemble(
        [
            [0.0, 0.0],
            [0.0, 0.0],
            [0.0, 0.0],
            [-2.03310E-06, -1.08674E-04],
            [-1.98080E-06, -9.06150E-05],
            [-1.92434E-06, -7.62786E-05],
            [-2.69634E-06, -7.62435E-05],
            [-3.07905E-06, -7.33397E-05],
            [-3.53877E-06, -7.13711E-05],
            [-2.63907E-06, -6.39554E-05],
            [-3.02147E-06, -6.16984E-05],
        ],
        [
            [0.0, 0.0],
            [0.0, 0.0],
            [0.0, 0.0],
            [1.24709E-07, -4.16439E-05],
            [1.35145E-07, -4.53102E-05],
            [1.49084E-07, -4.78475E-05],
            [1.40773E-07, -4.20202E-05],
            [1.43235E-07, -4.07701E-05],
            [1.46019E-07, -3.94319E-05],
            [1.55858E-07, -4.44431E-05],
            [1.58814E-07, -4.31588E-05],
        ],
        [
            [0.0, 0.0],
            [0.0, 0.0],
            [0.0, 0.0],
            [1.43035E-18, -5.46722E-16],
            [1.56896E-18, -5.94857E-16],
            [1.75422E-18, -6.28174E-16],
            [1.56896E-18, -5.51669E-16],
            [1.67897E-18, -5.35261E-16],
            [1.71665E-18, -5.17689E-16],
            [1.75422E-18, -5.83483E-16],
            [1.88528E-18, -5.66622E-16],
        ],
        [
            [0.0, 0.0],
            [0.0, 0.0],
            [0.0, 0.0],
            [2.12191E-07, -3.15597E-05],
            [2.26000E-07, -3.21435E-05],
            [2.39939E-07, -3.23776E-05],
            [2.48530E-07, -3.00119E-05],
            [2.61854E-07, -2.91929E-05],
            [2.74313E-07, -2.83041E-05],
            [2.64289E-07, -3.03509E-05],
            [2.79060E-07, -2.95626E-05],
        ],
        [
            0.0,
            0.0,
            0.0,
            8.09676E-07,
            8.58474E-07,
            9.03494E-07,
            7.01311E-07,
            6.17380E-07,
            5.16547E-07,
            7.44320E-07,
            6.59521E-07,
        ],
    )
}

/// Coolant-density derivatives \[per g/cm³\], referenced to 0.7125 g/cm³.
///
/// MATLAB `sigmavalues.coolden`. The axial reflector (material 1) has a
/// density derivative because it is water; the two radial reflector materials
/// do not.
///
/// # Errors
///
/// Cannot fail in practice: the internal assembly step only errors when the
/// absorption table is missing, and it is always supplied here.
pub fn coolant_density_derivatives() -> Result<SigmaSet> {
    assemble(
        [
            [7.45756E-02, 5.33634E-01],
            [0.0, 0.0],
            [0.0, 0.0],
            [1.35665E-01, 9.92628E-01],
            [1.35748E-01, 9.81985E-01],
            [1.35827E-01, 9.72267E-01],
            [1.31033E-01, 9.34697E-01],
            [1.29379E-01, 9.18171E-01],
            [1.27682E-01, 9.01293E-01],
            [1.31116E-01, 9.24925E-01],
            [1.29463E-01, 9.08456E-01],
        ],
        [
            [0.0, 0.0],
            [0.0, 0.0],
            [0.0, 0.0],
            [9.20694E-04, 2.47746E-02],
            [9.64160E-04, 3.14993E-02],
            [1.01410E-03, 3.81097E-02],
            [9.81951E-04, 3.51588E-02],
            [9.88437E-04, 3.63251E-02],
            [9.95175E-04, 3.74499E-02],
            [1.03522E-03, 4.20693E-02],
            [1.04291E-03, 4.33215E-02],
        ],
        [
            [0.0, 0.0],
            [0.0, 0.0],
            [0.0, 0.0],
            [1.02392E-14, 3.25255E-13],
            [1.08141E-14, 4.13542E-13],
            [1.14771E-14, 5.00328E-13],
            [1.08141E-14, 4.61715E-13],
            [1.11322E-14, 4.77078E-13],
            [1.12209E-14, 4.91900E-13],
            [1.14771E-14, 5.52387E-13],
            [1.18534E-14, 5.68857E-13],
        ],
        [
            [2.07688E-04, 7.58421E-03],
            [0.0, 0.0],
            [0.0, 0.0],
            [1.55185E-03, 2.52662E-02],
            [1.61491E-03, 2.86667E-02],
            [1.68015E-03, 3.19571E-02],
            [1.68397E-03, 3.14240E-02],
            [1.71972E-03, 3.24715E-02],
            [1.74989E-03, 3.35945E-02],
            [1.75528E-03, 3.49853E-02],
            [1.79499E-03, 3.61032E-02],
        ],
        [
            3.71310E-02,
            0.0,
            0.0,
            2.93195E-02,
            2.92696E-02,
            2.92154E-02,
            2.82489E-02,
            2.78895E-02,
            2.75202E-02,
            2.81877E-02,
            2.78259E-02,
        ],
    )
}

/// Cross-section increments for a fully inserted control rod \[1/cm per unit
/// rodded fraction\].
///
/// MATLAB `sigmavalues.crod`. The increments are the same for every material
/// except 6 (3.1 w/o fuel), which has its own set. `sigmavalupd3d_handler.m`
/// applies them against a reference of `0`, scaled by the fraction of the node
/// the rod occupies.
///
/// # Errors
///
/// Cannot fail in practice: the internal assembly step only errors when the
/// absorption table is missing, and it is always supplied here.
pub fn control_rod_increments() -> Result<SigmaSet> {
    // Materials 1-5 and 7-11 share one row; material 6 has its own.
    let common_tot = [3.73220E-03, -2.19926E-02];
    let m6_tot = [3.74092E-03, -1.67503E-02];
    let common_f = [-1.02786E-04, -2.82319E-03];
    let m6_f = [-1.22634E-04, -3.28086E-03];
    let common_fp = [-1.21448E-15, -3.70238E-14];
    let m6_fp = [-1.47557E-15, -4.30444E-14];
    let common_a = [2.47770E-03, 2.55875E-02];
    let m6_a = [2.42926E-03, 2.56478E-02];
    let common_down = -3.19253E-03;
    let m6_down = -3.14239E-03;

    let mut total = [common_tot; MATERIALS];
    let mut nu_fission = [common_f; MATERIALS];
    let mut kappa_fission = [common_fp; MATERIALS];
    let mut absorption = [common_a; MATERIALS];
    let mut down = [common_down; MATERIALS];
    total[5] = m6_tot;
    nu_fission[5] = m6_f;
    kappa_fission[5] = m6_fp;
    absorption[5] = m6_a;
    down[5] = m6_down;

    assemble(total, nu_fission, kappa_fission, absorption, down)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_tables_have_eleven_materials_and_two_groups() {
        let s = base_sigmas().expect("assembles");
        assert_eq!(s.materials(), 11);
        assert_eq!(s.ngroups(), 2);
        assert_eq!(s.total[0][0], 0.0532058);
        assert_eq!(s.total[10][1], 0.764704);
    }

    /// The three reflector materials do not fission; the eight fuel materials
    /// do, in both groups.
    #[test]
    fn only_the_fuel_materials_fission() {
        let s = base_sigmas().expect("assembles");
        for m in 0..3 {
            assert_eq!(
                s.nu_fission[m].iter().sum::<f64>(),
                0.0,
                "material {}",
                m + 1
            );
        }
        for m in 3..11 {
            assert!(s.nu_fission[m][1] > 0.0, "material {}", m + 1);
            assert!(s.kappa_fission[m][1] > 0.0, "material {}", m + 1);
        }
    }

    /// The scattering diagonal is closed on total minus absorption minus
    /// out-scatter, so every group's balance is exact by construction.
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
                    "material {} group {}: {balance} vs {}",
                    m + 1,
                    g + 1,
                    s.total[m][g]
                );
            }
        }
    }

    /// Physical signs of the feedback derivatives: more boron absorbs more,
    /// hotter fuel broadens resonances (thermal fission down), denser coolant
    /// moderates better.
    #[test]
    fn feedback_derivatives_have_the_expected_signs() {
        let boron = boron_derivatives().expect("assembles");
        assert!(
            boron.absorption[3][1] > 0.0,
            "boron adds thermal absorption"
        );
        assert!(
            boron.nu_fission[3][1] < 0.0,
            "boron suppresses thermal fission"
        );

        let doppler = fuel_temperature_derivatives().expect("assembles");
        assert!(
            doppler.absorption[3][0] > 0.0,
            "Doppler raises fast absorption"
        );
        assert!(doppler.nu_fission[3][1] < 0.0);

        let density = coolant_density_derivatives().expect("assembles");
        assert!(
            density.nu_fission[3][1] > 0.0,
            "denser coolant moderates more"
        );

        let rod = control_rod_increments().expect("assembles");
        assert!(rod.absorption[3][1] > 0.0, "a rod adds thermal absorption");
        assert!(rod.nu_fission[3][1] < 0.0);
    }

    /// Material 6 is the one control-rod row that differs.
    #[test]
    fn control_rod_material_six_differs_from_the_rest() {
        let rod = control_rod_increments().expect("assembles");
        assert_eq!(rod.total[5], vec![3.74092E-03, -1.67503E-02]);
        assert_eq!(rod.total[4], vec![3.73220E-03, -2.19926E-02]);
        assert_eq!(rod.total[6], vec![3.73220E-03, -2.19926E-02]);
    }

    /// Every feedback table covers all eleven materials, so a node of any
    /// composition can be updated.
    #[test]
    fn every_feedback_table_covers_every_material() {
        for set in [
            boron_derivatives().expect("assembles"),
            fuel_temperature_derivatives().expect("assembles"),
            coolant_temperature_derivatives().expect("assembles"),
            coolant_density_derivatives().expect("assembles"),
            control_rod_increments().expect("assembles"),
        ] {
            assert_eq!(set.materials(), MATERIALS);
            assert_eq!(set.ngroups(), GROUPS);
        }
    }
}
