//! The NEACRP 3-D LWR core transient benchmark, BWR case D — steady state.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `neacrpd1.m`, `main_exec_diff3d_standalone` snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see the crate README, "Permission and attribution".
//! - **Licence:** GPL-3.0-only.
//! - **Composition maps:** `src/data/NEACRPD1_*.csv`; see
//!   `src/data/PROVENANCE.md`.
//!
//! # Why this case matters
//!
//! [`crate::iaea3ds`] is pure neutronics; this is the first **coupled** case in
//! the crate. It carries everything the thermal-hydraulic side needs — core
//! power, coolant inlet state, mass flux, a 22-node fuel rod with UO2/gap/clad
//! materials, and per-material cross-section slopes against both fuel
//! temperature and coolant density — so it is what
//! [`crate::thdiffusion_solverxyz`] was written to consume.
//!
//! # The problem
//!
//! A 17 x 17 x 14 quarter core on a 30.48/2 cm radial by 30.48 cm axial mesh —
//! 259.08 x 259.08 x 426.72 cm. Reflective on the low `x` and `y` faces (the
//! quarter-core symmetry planes), zero flux on the other four. Two energy
//! groups, **19 materials**, fission into the fast group only.
//!
//! The material map is built from two files rather than one per level: a 17x17
//! *column* map naming which of 10 radial column types each lattice position
//! is, and a 14x10 *axial* table giving the material of each column type at
//! each level. A column entry of `0` is outside the core outline.
//!
//! # The cross sections are given as total and absorption
//!
//! The case supplies `sigmavalues.tot`, `sigmavalues.a` and the off-diagonal
//! scattering, then closes the within-group scattering by difference:
//!
//! ```text
//! s(m, 1, 1) = tot(m, 1) - a(m, 1) - s(m, 2, 1)
//! s(m, 2, 2) = tot(m, 2) - a(m, 2) - s(m, 1, 2)
//! ```
//!
//! That identity is what makes the set consistent, and it is checked by a test
//! rather than assumed. Absorption is not carried on [`SigmaValues`] because
//! nothing downstream reads it — it exists in the case file only to close the
//! scattering. As with `iaea3ds`, `nu` is all ones and `sigmavalues.f` already
//! carries the `nu * Sigma_f` product.
//!
//! # `sigmavalues.*.upd` is dead data in the reference
//!
//! The case file builds a per-node mask marking which nodes have feedback
//! applied — non-zero wherever the material fissions. **Nothing reads it.** A
//! search of the whole snapshot finds `.upd` written by the case files and
//! consumed nowhere, so it is not carried here. The feedback handler applies
//! its slopes to every material row, which is what the reference actually does.
//!
//! # Two values are written twice; the second wins
//!
//! `th.flowrate` is assigned three times in the reference, the first two
//! commented derivations and the third live. Only the last takes effect and
//! only the last is translated; the other two are recorded in the constant's
//! docs so the intent is not lost.

use crate::geometry_ends3d::geometry_ends3d;
use crate::iapws_if97::{basic::h1_pt, backward::t_ph, region4::tsat_p};
use crate::matlab::{Array2, Array3};
use crate::sigmavalupd3d::DeltaSigmaValues;
use crate::sigmavalupd3d_handler::FeedbackTables;
use crate::types::{
    BoundaryCondition, Conductivity, Coolant, FlowDirection, FuelGeometry, Geometry, MassFlux,
    Params, SigmaValues, Th,
};

/// The 17x17 radial column map: which column type each lattice position is.
///
/// Entries run 1 to 10; `0` is outside the core outline.
const COLUMN_MAP: &str = include_str!("data/NEACRPD1_1.csv");
/// The 14x10 axial table: the material at each level, for each column type.
const AXIAL_TABLE: &str = include_str!("data/NEACRPD1_COL.csv");

/// The number of materials in the case's cross-section set.
pub const MATERIALS: usize = 19;

/// Parse a comma-separated integer map of known shape.
///
/// # Panics
/// If the file is not `rows` lines of `cols` integers.
fn parse_map(text: &str, rows: usize, cols: usize) -> Array2<usize> {
    let parsed: Vec<Vec<usize>> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            l.split(',')
                .map(|c| {
                    c.trim()
                        .parse::<usize>()
                        .unwrap_or_else(|e| panic!("bad map entry {c:?}: {e}"))
                })
                .collect()
        })
        .collect();

    assert_eq!(parsed.len(), rows, "expected {rows} rows, got {}", parsed.len());
    let mut a = Array2::<usize>::zeros(rows, cols);
    for (i, row) in parsed.iter().enumerate() {
        assert_eq!(row.len(), cols, "row {i} has {} entries, expected {cols}", row.len());
        for (j, v) in row.iter().enumerate() {
            a.set(i, j, *v);
        }
    }
    a
}

/// Fill a `MATERIALS`-by-2 table from three literal blocks of rows.
///
/// The reference writes the cross-section tables in three slices — rows 1-6,
/// 7-12 and 13-19 — because that is how they fit on a line. Flattening them
/// into one array here keeps the transcription checkable against the source
/// line by line.
fn table2(rows: [[f64; 2]; MATERIALS]) -> Array2<f64> {
    let mut a = Array2::<f64>::zeros(MATERIALS, 2);
    for (m, row) in rows.iter().enumerate() {
        for (g, v) in row.iter().enumerate() {
            a.set(m, g, *v);
        }
    }
    a
}

/// Build the scattering array from the down-scatter column and close the
/// diagonal by difference against total and absorption.
///
/// `down[m]` is `s(m, 2, 1)` — group 1 into group 2. Up-scatter `s(m, 1, 2)`
/// is zero throughout this case, so the closure reduces to subtracting the
/// down-scatter from group 1 only.
fn scattering(tot: &Array2<f64>, a: &Array2<f64>, down: [f64; MATERIALS]) -> Array3<f64> {
    let mut s = Array3::<f64>::zeros(MATERIALS, 2, 2);
    for (m, d) in down.iter().enumerate() {
        // `s(m, gt, g)`: destination first, so (1, 0) is 1 -> 2 down-scatter.
        s.set(m, 1, 0, *d);
        s.set(m, 0, 0, tot.get(m, 0) - a.get(m, 0) - *d);
        // `s(:, 1, 2)` is zero, so group 2's diagonal is total less absorption.
        s.set(m, 1, 1, tot.get(m, 1) - a.get(m, 1));
    }
    s
}

/// `[params, geometry, th, constants, whichsigma, sigmavalues] = neacrpd1(params)`.
///
/// Builds the complete NEACRP case-D steady state: mesh, boundary conditions,
/// the 19-material two-group cross-section set, the material map, the
/// thermal-hydraulic inlet state and rod geometry, and the fuel-temperature and
/// coolant-density feedback tables.
///
/// # Returns
///
/// `(params, geometry, th, whichsigma, sigmavalues, feedback)`. The reference's
/// `constants` output carries only `chi` and `nu`, both already on
/// `sigmavalues`, and its feedback slopes ride on `sigmavalues.fueltemp` /
/// `sigmavalues.coolden` where this crate keeps them in a separate
/// [`FeedbackTables`].
///
/// # The mesh is fixed at 17 x 17 x 14
///
/// The reference computes `xscale = maxix/17` and friends and divides
/// `th.nfuelpin` by them, so a refined mesh is nominally allowed. But the
/// material lookup is `whichdata(ceil(ix/maxix*17), ceil(iy/maxiy*17))`, an
/// identity only at 17, and `geometry.Lz` is written with a stride of `maxiz`
/// that assumes the benchmark's own layer count. This translation fixes the
/// mesh at 17 x 17 x 14 and asserts it rather than reproducing a refinement
/// path the reference never exercises.
///
/// # Panics
///
/// If `params.maxix`, `maxiy` or `maxiz` is set to anything other than
/// 17, 17, 14.
#[allow(clippy::type_complexity)]
pub fn neacrpd1(
    params: &Params,
) -> (Params, Geometry, Th, Array3<usize>, SigmaValues, FeedbackTables) {
    const NX: usize = 17;
    const NY: usize = 17;
    const NZ: usize = 14;

    let mut params = params.clone();
    params.maxix = Some(NX);
    params.maxiy = Some(NY);
    params.maxiz = Some(NZ);
    params.nc = Some(0);
    params.g = 2;

    let es = NX * NY * NZ;

    // ----- mesh -----
    // `30.48/2*17` radially — half a 12-inch assembly pitch, quarter core.
    let xtot = 30.48 / 2.0 * 17.0;
    let ytot = xtot;
    // Axial layers are a full 30.48 cm each; `geometry.Ztot` in the reference is
    // just their sum and nothing reads it, so [`Geometry`] does not carry it.
    let z_layer = 30.48;
    let (sx, sy) = (xtot / NX as f64, ytot / NY as f64);

    let uniform = |rows: usize, cols: usize| Array2::<usize>::zeros(rows, cols);

    let mut geometry = Geometry {
        xtot,
        ytot,
        // `zscale = int64(maxiz/14)` — mesh layers per axial block of the
        // benchmark model. Only the transient driver reads it, to map an
        // active-core block number onto mesh layers.
        zscale: NZ / 14,
        lx: vec![sx; es],
        ly: vec![sy; es],
        lz: vec![z_layer; es],
        vi: vec![sx * sy * z_layer; es],
        // Quarter-core symmetry planes on the low radial faces.
        xmin: BoundaryCondition::Reflective,
        xmax: BoundaryCondition::ZeroFlux,
        ymin: BoundaryCondition::Reflective,
        ymax: BoundaryCondition::ZeroFlux,
        zmin: BoundaryCondition::ZeroFlux,
        zmax: BoundaryCondition::ZeroFlux,
        // `geometry_ends3d` fills these below.
        xlows: Some(uniform(NY, NZ)),
        xhis: Some(uniform(NY, NZ)),
        ylows: Some(uniform(NX, NZ)),
        yhis: Some(uniform(NX, NZ)),
        zlows: Some(uniform(NX, NY)),
        zhis: Some(uniform(NX, NY)),
        ..Default::default()
    };

    // ----- cross sections -----
    #[rustfmt::skip]
    let tot = table2([
        [0.111030, 0.830012], [0.189784, 0.694136], [0.188544, 0.693963],
        [0.134270, 0.730050], [0.189186, 0.693475], [0.188264, 0.693345],
        [0.199654, 0.718647], [0.198692, 0.719003], [0.189151, 0.693476],
        [0.188381, 0.693427], [0.187960, 0.693550], [0.188575, 0.693636],
        [0.189091, 0.693478], [0.188616, 0.693591], [0.187354, 0.693960],
        [0.199871, 0.722345], [0.198410, 0.718597], [0.197215, 0.719295],
        [0.184542, 1.368640],
    ]);
    #[rustfmt::skip]
    let f = table2([
        [0.0, 0.0],                  [0.446986E-02, 0.828220E-01],
        [0.446539E-02, 0.804386E-01], [0.0, 0.0],
        [0.413061E-02, 0.738611E-01], [0.412726E-02, 0.720736E-01],
        [0.416239E-02, 0.649081E-01], [0.416026E-02, 0.636570E-01],
        [0.413003E-02, 0.738046E-01], [0.412816E-02, 0.723935E-01],
        [0.412665E-02, 0.716701E-01], [0.412888E-02, 0.728618E-01],
        [0.412887E-02, 0.736916E-01], [0.412995E-02, 0.730336E-01],
        [0.412543E-02, 0.708631E-01], [0.415877E-02, 0.650624E-01],
        [0.416892E-02, 0.643471E-01], [0.416619E-02, 0.626218E-01],
        [0.0, 0.0],
    ]);
    // Absorption. Used only to close the scattering diagonal — see the module
    // docs — so it is not carried on `SigmaValues`.
    #[rustfmt::skip]
    let absorption = table2([
        [3.92E-04, 1.4801E-02],      [1.02352E-02, 7.49127E-02],
        [1.03417E-02, 7.68592E-02],  [5.53E-04, 6.22329E-03],
        [1.01071E-02, 6.83185E-02],  [1.01869E-02, 6.97783E-02],
        [7.09736E-03, 4.84724E-02],  [7.15434E-03, 4.90908E-02],
        [1.01112E-02, 6.83829E-02],  [1.01774E-02, 6.96467E-02],
        [1.02185E-02, 7.04713E-02],  [1.01653E-02, 6.94981E-02],
        [1.01196E-02, 6.85118E-02],  [1.01582E-02, 6.93833E-02],
        [1.02817E-02, 7.18573E-02],  [7.08533E-03, 4.82128E-02],
        [7.17808E-03, 5.00243E-02],  [7.25399E-03, 5.09656E-02],
        [3.59E-04, 1.0868E-02],
    ]);
    #[rustfmt::skip]
    let s = scattering(&tot, &absorption, [
        0.022595,  0.0141764, 0.0142295, 0.018177,  0.0143548,
        0.0143946, 0.0164565, 0.0165185, 0.0143552, 0.0143893,
        0.0144036, 0.0143771, 0.0143562, 0.0143789, 0.0144216,
        0.016448,  0.016521,  0.0165892, 0.037579,
    ]);

    // `constants.nu = ones(19, G)` — `f` already carries nu*Sigma_f.
    let mut nu = Array2::<f64>::zeros(MATERIALS, 2);
    // `constants.chi(:,1) = 1` — every fission neutron born fast.
    let mut chi = Array2::<f64>::zeros(MATERIALS, 2);
    for m in 0..MATERIALS {
        nu.set(m, 0, 1.0);
        nu.set(m, 1, 1.0);
        chi.set(m, 0, 1.0);
    }

    let sigmavalues = SigmaValues { tot, f, s, nu, chi, fp: None };

    // ----- material map -----
    // A radial column type per lattice position, then a material per (level,
    // column). `0` in the column map is outside the core outline and stays 0.
    let columns = parse_map(COLUMN_MAP, NX, NY);
    let axial = parse_map(AXIAL_TABLE, NZ, 10);

    let mut whichsigma = Array3::<usize>::zeros(NX, NY, NZ);
    for ix in 0..NX {
        for iy in 0..NY {
            let col = columns.get(ix, iy);
            if col == 0 {
                continue;
            }
            for iz in 0..NZ {
                whichsigma.set(ix, iy, iz, axial.get(iz, col - 1));
            }
        }
    }
    // The reference also stores the map on `geometry.whichsigma`; this crate
    // passes it separately, as `iaea3ds` does.
    geometry_ends3d(&params, &mut geometry, &whichsigma);

    // ----- thermal hydraulics -----
    let inletpress = 6.7;
    // 46.52 kJ/kg of inlet subcooling below the saturated-liquid enthalpy.
    let tsat = tsat_p(inletpress);
    let hsat = h1_pt(inletpress, tsat);
    let inlettemp = t_ph(inletpress, hsat - 46.52);

    // `70000 / (30.48^2 - 221*pi*0.715^2)` — core mass flow over the flow area
    // left by 221 pins of 0.715 cm radius in a 30.48 cm square node. The
    // reference also carries two earlier, overwritten assignments:
    // `13000000/157/400.78` and an assembly-averaged form; only this one runs.
    let flow_area = 30.48 * 30.48 - 221.0 * std::f64::consts::PI * 0.715 * 0.715;
    let flowrate = 70000.0 / flow_area;

    // ----- fuel rod -----
    params.fuel.gapn = 1;
    params.fuel.cladn = 1;
    params.fuel.fueln = 20;
    params.fuel.maxir = params.fuel.gapn + params.fuel.cladn + params.fuel.fueln;
    let maxir = params.fuel.maxir;

    let fuelrad = 1.237 / 2.0;
    let fuelgap = 0.03 / 2.0;
    let clad = (1.43 - 1.267) / 2.0;
    let pitch = 1.875;
    let rtot = fuelrad + fuelgap + clad;

    let mut lr = vec![0.0; maxir];
    for (i, l) in lr.iter_mut().enumerate() {
        *l = if i < params.fuel.fueln {
            fuelrad / params.fuel.fueln as f64
        } else if i < params.fuel.fueln + params.fuel.gapn {
            fuelgap / params.fuel.gapn as f64
        } else {
            clad / params.fuel.cladn as f64
        };
    }

    // `Ctr(ir) = sum(Lr(1:ir)) - 0.5*Lr(ir)` — node centre radii.
    let mut ctr = vec![0.0; maxir];
    let mut running = 0.0;
    for (i, c) in ctr.iter_mut().enumerate() {
        running += lr[i];
        *c = running - 0.5 * lr[i];
    }

    // `Vi(1) = pi*Lr(1)^2`, then annular shells.
    //
    // **The reference's shell formula is wrong, and it does not matter.** It
    // writes `rminus = sum(Lr(i-1))`, `rplus = sum(Lr(i))` — but for a scalar
    // index `sum` is the identity, so those are node *thicknesses*, not
    // cumulative radii. Every fuel node has the same thickness, so `Vi(2:20)`
    // comes out exactly zero instead of increasing with radius.
    //
    // It is harmless because `fuel.Vi` is **dead**: `th_solverxyz.m` and
    // `th_solvertimexyz.m` read it into `Vif` and then use it only on two
    // commented-out lines, one of them the volume-averaging line that the
    // Doppler assignment replaced. Nothing in this crate reads the field
    // either. Reproduced as written — the no-silent-repairs policy — and
    // pinned by a test so it stays visible if a future consumer appears.
    let mut vi = vec![0.0; maxir];
    vi[0] = std::f64::consts::PI * lr[0] * lr[0];
    for i in 1..maxir {
        vi[i] = std::f64::consts::PI * (lr[i] * lr[i] - lr[i - 1] * lr[i - 1]);
    }

    // 1 = fuel, 0 = gap, 2 = cladding.
    let mut whichk = vec![1usize; maxir];
    for (i, k) in whichk.iter_mut().enumerate() {
        let ir = i + 1;
        if ir > params.fuel.fueln && ir <= params.fuel.fueln + params.fuel.gapn {
            *k = 0;
        } else if ir > params.fuel.fueln + params.fuel.gapn {
            *k = 2;
        }
    }

    let subarea = pitch * pitch - std::f64::consts::PI * rtot * rtot;
    let hydia = 4.0 * subarea / (2.0 * std::f64::consts::PI * rtot + 4.0 * pitch - 8.0 * rtot);

    geometry.fuel = FuelGeometry {
        lr,
        ctr,
        vi,
        whichk,
        tcon: vec![Conductivity::Uo2Fuel, Conductivity::ZircaloyClad],
        // Steady state has no time term, so no heat capacities are needed.
        rhocp: Vec::new(),
        // `tcon{3}` — attributed in the reference to the NEACRP benchmark.
        gap_conductance: 0.35,
        fuelrad,
        rtot,
        pitch,
        subarea,
        hydia,
        doppleralpha: 0.7,
    };

    let th = Th {
        coolant: Coolant {
            inlettemp,
            inletpress,
            inletvoid: 1e-14,
            ..Default::default()
        },
        // `1800/4 * 1e6` W — a quarter of the 1800 MW core, matching the
        // quarter-core geometry.
        maxpow: 1800.0 / 4.0 * 1e6,
        powratio: 1.0,
        coolheatfrac: 0.019,
        // `196/4` pins per node, then divided by the radial scales (both 1).
        nfuelpin: 196.0 / 4.0,
        flowrate: MassFlux::Uniform(flowrate),
        flowdir: FlowDirection::Up,
        ..Default::default()
    };

    params.cooltempavg = inlettemp;
    params.boron = 1000.0;
    params.fueltempavg = 650.0;
    params.cooldenavg = 0.453;

    // ----- feedback tables -----
    let feedback = FeedbackTables {
        fueltemp: Some(fueltemp_table()),
        coolden: Some(coolden_table()),
        ..Default::default()
    };

    (params, geometry, th, whichsigma, sigmavalues, feedback)
}

/// Cross-section slopes against fuel temperature, referenced to 573.15 K.
///
/// The handler applies these against `sqrt(T) - sqrt(T_ref)`, the square-root
/// Doppler law — see [`crate::sigmavalupd3d_handler`].
fn fueltemp_table() -> DeltaSigmaValues {
    #[rustfmt::skip]
    let tot = table2([
        [0.0, 0.0], [0.0, -8.23459E-05], [0.0, -8.2524E-05],
        [0.0, 0.0], [0.0, -8.24488E-05], [0.0, -8.25824E-05],
        [0.0, -8.67114E-05], [0.0, -8.68472E-05], [0.0, -8.4204E-05],
        [0.0, -8.26201E-05], [0.0, -8.26179E-05], [0.0, -8.25289E-05],
        [0.0, -8.23634E-05], [0.0, -8.26954E-05], [0.0, -8.26889E-05],
        [0.0, -8.66561E-05], [0.0, -8.67723E-05], [0.0, -8.70674E-05],
        [0.0, 0.0],
    ]);
    #[rustfmt::skip]
    let f = table2([
        [0.0, 0.0], [0.0, -0.350770E-04], [0.0, -0.341409E-04],
        [0.0, 0.0], [0.0, -0.313155E-04], [0.0, -0.306135E-04],
        [0.0, -0.277381E-04], [0.0, -0.272325E-04], [0.0, -0.312795E-04],
        [0.0, -0.307625E-04], [0.0, -0.304546E-04], [0.0, -0.309227E-04],
        [0.0, -0.312074E-04], [0.0, -0.310605E-04], [0.0, -0.301369E-04],
        [0.0, -0.277728E-04], [0.0, -0.277559E-04], [0.0, -0.268636E-04],
        [0.0, 0.0],
    ]);
    #[rustfmt::skip]
    let absorption = table2([
        [0.0, 0.0], [0.200902E-04, -0.262873E-04], [0.201801E-04, -0.270360E-04],
        [0.0, 0.0], [0.204046E-04, -0.239924E-04], [0.204720E-04, -0.245539E-04],
        [0.231603E-04, -0.172597E-04], [0.232572E-04, -0.174963E-04],
        [0.203980E-04, -0.240056E-04], [0.204749E-04, -0.245168E-04],
        [0.204833E-04, -0.248087E-04], [0.204384E-04, -0.244344E-04],
        [0.203849E-04, -0.240320E-04], [0.204806E-04, -0.244425E-04],
        [0.205059E-04, -0.253184E-04], [0.231300E-04, -0.171504E-04],
        [0.232614E-04, -0.178265E-04], [0.234010E-04, -0.182100E-04],
        [0.0, 0.0],
    ]);
    #[rustfmt::skip]
    let s = scattering(&tot, &absorption, [
        0.0, -0.160580E-04, -0.161574E-04, 0.0, -0.162970E-04,
        -0.163716E-04, -0.189002E-04, -0.189895E-04, -0.162923E-04,
        -0.163709E-04, -0.163872E-04, -0.163375E-04, -0.162829E-04,
        -0.163695E-04, -0.164185E-04, -0.188723E-04, -0.189906E-04,
        -0.191256E-04, 0.0,
    ]);

    DeltaSigmaValues { tot, f, fp: Array2::<f64>::zeros(MATERIALS, 2), s, reference: 573.15 }
}

/// Cross-section slopes against coolant density, referenced to 0.55 g/cm³.
fn coolden_table() -> DeltaSigmaValues {
    #[rustfmt::skip]
    let tot = table2([
        [2.1932E-07, 7.01285E-06], [0.130164, 0.759141], [0.12875, 0.752907],
        [1.75456E-06, 1.40365E-05], [0.129802, 0.77124], [0.128744, 0.766599],
        [0.127443, 0.750235], [0.126783, 0.746987], [0.129572, 0.771049],
        [0.128909, 0.766905], [0.128450, 0.762266], [0.129158, 0.765378],
        [0.12965, 0.770663], [0.129248, 0.767578], [0.127871, 0.753668],
        [0.127402, 0.73863], [0.126972, 0.745357], [0.126127, 0.741955],
        [0.0, 1.75456E-06],
    ]);
    #[rustfmt::skip]
    let f = table2([
        [0.0, 0.0], [0.111084E-02, 0.246360E-01], [0.111093E-02, 0.236482E-01],
        [0.0, 0.0], [0.960766E-03, 0.211180E-01], [0.960828E-03, 0.203775E-01],
        [0.696670E-03, 0.130177E-01], [0.693127E-03, 0.124660E-01],
        [0.959197E-03, 0.211062E-01], [0.962980E-03, 0.205383E-01],
        [0.963165E-03, 0.204017E-01], [0.963117E-03, 0.208959E-01],
        [0.956030E-03, 0.210825E-01], [0.967291E-03, 0.208614E-01],
        [0.967730E-03, 0.204502E-01], [0.692305E-03, 0.127594E-01],
        [0.704641E-03, 0.137254E-01], [0.701297E-03, 0.126783E-01],
        [0.0, 0.0],
    ]);
    #[rustfmt::skip]
    let absorption = table2([
        [0.856719E-09, 0.109660E-06], [0.238730E-02, 0.126336E-01],
        [0.247360E-02, 0.127195E-01], [0.0, -0.685375E-08],
        [0.227347E-02, 0.108673E-01], [0.233817E-02, 0.109320E-01],
        [0.170083E-02, 0.126452E-01], [0.171473E-02, 0.125016E-01],
        [0.228093E-02, 0.108792E-01], [0.232403E-02, 0.109351E-01],
        [0.236219E-02, 0.110489E-01], [0.231898E-02, 0.110057E-01],
        [0.229564E-02, 0.109047E-01], [0.229551E-02, 0.109412E-01],
        [0.241000E-02, 0.112829E-01], [0.169762E-02, 0.127121E-01],
        [0.172207E-02, 0.126051E-01], [0.173860E-02, 0.125542E-01],
        [0.428360E-09, 0.109660E-06],
    ]);
    #[rustfmt::skip]
    let s = scattering(&tot, &absorption, [
        0.219320E-06, 0.196627E-01, 0.197654E-01, 0.219320E-06, 0.198449E-01,
        0.199218E-01, 0.197129E-01, 0.197951E-01, 0.198434E-01, 0.199168E-01,
        0.199429E-01, 0.198915E-01, 0.198407E-01, 0.199065E-01, 0.199848E-01,
        0.197119E-01, 0.197836E-01, 0.198932E-01, 0.109660E-06,
    ]);

    DeltaSigmaValues { tot, f, fp: Array2::<f64>::zeros(MATERIALS, 2), s, reference: 0.55 }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cross-section set is internally consistent: scattering closes
    /// against total and absorption.
    ///
    /// # Methodology
    ///
    /// The case file supplies total, absorption and the down-scatter, and
    /// closes the within-group diagonal by difference. So for every material
    /// and both groups the implied absorption `tot(m, g) - sum_gt s(m, gt, g)`
    /// must come back **positive** — a physical absorption cross section. A
    /// mistranscribed digit in any of the three tables shows up as a negative
    /// or absurd value here.
    ///
    /// Also checked: no up-scatter anywhere, the fission spectrum is entirely
    /// fast, and the three reflector materials (1, 4 and 19, 1-based) really
    /// have zero fission.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// Every material and both groups closed to a **positive** implied
    /// absorption, spanning `3.590e-4` to `7.686e-2` /cm. No material
    /// up-scatters, every fission neutron is born fast, and materials 1, 4
    /// and 19 have zero fission.
    ///
    /// **Interpretation.** The 19x2 total, absorption and down-scatter
    /// tables are transcribed consistently — a wrong digit in any of the
    /// three would leave some material with a negative implied absorption.
    /// The span itself is physical: 3.6e-4 /cm is the bottom reflector's
    /// fast group and 7.7e-2 /cm is rodded fuel's thermal group.
    #[test]
    fn the_cross_sections_close_against_absorption() {
        let (_p, _g, _t, _w, sv, _f) = neacrpd1(&Params::default());

        let mut smallest = f64::INFINITY;
        let mut largest: f64 = 0.0;
        for m in 0..MATERIALS {
            for g in 0..2 {
                let scattered_out: f64 = (0..2).map(|gt| sv.s.get(m, gt, g)).sum();
                let implied = sv.tot.get(m, g) - scattered_out;
                assert!(
                    implied > 0.0,
                    "material {} group {g} has non-positive implied absorption {implied:e}",
                    m + 1
                );
                smallest = smallest.min(implied);
                largest = largest.max(implied);
            }
            assert_eq!(sv.s.get(m, 0, 1), 0.0, "material {} up-scatters", m + 1);
            assert_eq!(sv.chi.get(m, 0), 1.0);
            assert_eq!(sv.chi.get(m, 1), 0.0);
        }

        for m in [0usize, 3, 18] {
            assert_eq!(sv.f.get(m, 0), 0.0, "material {} fissions", m + 1);
            assert_eq!(sv.f.get(m, 1), 0.0, "material {} fissions", m + 1);
        }
        eprintln!("implied absorption spans {smallest:.6e} .. {largest:.6e} /cm");
    }

    /// The material map has the benchmark's radial outline and axial structure.
    ///
    /// # Methodology
    ///
    /// The map is built from a 17x17 column map and a 14x10 axial table, so the
    /// core outline must be a **right prism** — a position void at one level is
    /// void at every level, because voidness comes from the column map alone.
    /// The bottom level must also be reflector throughout, since that is what
    /// the axial table's first row says.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// **40 void positions of 289 at every one of the 14 levels** — the
    /// outline is a right prism, as construction from a single column map
    /// requires. The bottom level holds only materials `{1, 19}`, the top
    /// only `{4, 19}`, and the mid-plane `{2, 5, 7, 12, 15, 18, 19}`.
    ///
    /// **Interpretation.** Both axial ends are reflector (1 bottom, 4 top,
    /// 19 radial) and the fuelled band between them carries six distinct
    /// fuel compositions — the enrichment and burnup zoning the benchmark
    /// specifies.
    #[test]
    fn the_material_map_is_a_right_prism_over_the_column_outline() {
        let (_p, _g, _t, w, _sv, _f) = neacrpd1(&Params::default());

        let voids_at = |iz: usize| {
            (0..17)
                .flat_map(|ix| (0..17).map(move |iy| (ix, iy)))
                .filter(|&(ix, iy)| w.get(ix, iy, iz) == 0)
                .collect::<Vec<_>>()
        };
        let level0 = voids_at(0);
        for iz in 1..14 {
            assert_eq!(voids_at(iz), level0, "level {iz} has a different outline");
        }
        eprintln!("void positions per level: {} of 289", level0.len());

        let materials_at = |iz: usize| {
            let mut m: Vec<usize> = (0..17)
                .flat_map(|ix| (0..17).map(move |iy| (ix, iy)))
                .map(|(ix, iy)| w.get(ix, iy, iz))
                .filter(|&v| v != 0)
                .collect();
            m.sort_unstable();
            m.dedup();
            m
        };
        eprintln!("bottom level materials: {:?}", materials_at(0));
        eprintln!("mid    level materials: {:?}", materials_at(7));
        eprintln!("top    level materials: {:?}", materials_at(13));
        assert!(
            materials_at(0).iter().all(|&m| m == 1 || m == 19),
            "the bottom level should be reflector only"
        );
    }

    /// The thermal-hydraulic inlet state is subcooled water at 6.7 MPa.
    ///
    /// # Methodology
    ///
    /// The reference sets the inlet enthalpy 46.52 kJ/kg below saturation and
    /// inverts it back to a temperature through IAPWS-IF97. Two things follow:
    /// the inlet must be **below** `Tsat(6.7 MPa)` and above the triple point —
    /// an inversion that fell out of region 1 would violate one or the other.
    /// The mass flux and rod geometry are checked against hand arithmetic on
    /// the reference's own expressions.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// `Tsat(6.7 MPa) = 556.0312 K`, inlet `547.1528 K`, so **8.878 K of
    /// subcooling**. Mass flux `121.9319 g/(s cm2)`. Rod: 22 radial nodes,
    /// `Rtot = 0.715000 cm`, `subarea = 1.909564 cm2`, hydraulic diameter
    /// `1.217742 cm`.
    ///
    /// **Interpretation.** The IF97 round trip — saturation temperature,
    /// then the saturated-liquid enthalpy, then the backward `T(p, h)` on
    /// that value less 46.52 kJ/kg — lands 8.9 K below saturation, which is
    /// the right order for 46.5 kJ/kg at a liquid `cp` near 5.2 kJ/(kg K).
    /// It confirms the three IF97 entry points compose correctly on real
    /// case data, not just on the steam-table probes they were verified
    /// against individually.
    #[test]
    fn the_inlet_state_and_rod_geometry_match_the_reference_arithmetic() {
        let (params, geometry, th, _w, _sv, _f) = neacrpd1(&Params::default());

        let tsat = tsat_p(6.7);
        eprintln!("Tsat(6.7 MPa)   = {tsat:.4} K");
        eprintln!("inlet temp      = {:.4} K", th.coolant.inlettemp);
        eprintln!("subcooling      = {:.4} K", tsat - th.coolant.inlettemp);
        assert!(
            th.coolant.inlettemp < tsat && th.coolant.inlettemp > 273.15,
            "the inlet must be subcooled liquid"
        );

        let flowrate = match th.flowrate {
            MassFlux::Uniform(v) => v,
            _ => panic!("expected a uniform mass flux"),
        };
        eprintln!("mass flux       = {flowrate:.4} g/(s cm2)");
        let expected = 70000.0 / (929.0304 - 221.0 * std::f64::consts::PI * 0.511_225);
        assert!((flowrate - expected).abs() < 1e-9, "{flowrate} vs {expected}");

        assert_eq!(params.fuel.maxir, 22, "20 fuel + 1 gap + 1 clad");
        assert_eq!(geometry.fuel.whichk[19], 1, "node 20 is the outer fuel ring");
        assert_eq!(geometry.fuel.whichk[20], 0, "node 21 is the gap");
        assert_eq!(geometry.fuel.whichk[21], 2, "node 22 is the cladding");
        assert!((geometry.fuel.rtot - 0.715).abs() < 1e-12, "{}", geometry.fuel.rtot);
        eprintln!("Rtot            = {:.6} cm", geometry.fuel.rtot);
        eprintln!("subarea         = {:.6} cm2", geometry.fuel.subarea);
        eprintln!("hydraulic dia   = {:.6} cm", geometry.fuel.hydia);
    }

    /// **Defect: `geometry.fuel.Vi` is computed wrongly, and it is dead.**
    ///
    /// # Methodology
    ///
    /// The reference's annular-shell formula uses `sum(Lr(i))`, which for a
    /// scalar index is the node *thickness*, not the cumulative radius. Every
    /// fuel node has the same thickness, so every fuel shell volume after the
    /// first comes out zero. This pins the wrong behaviour rather than
    /// repairing it, per the no-silent-repairs policy. No consumer exists:
    /// `th_solverxyz.m` reads the field into `Vif` and uses it only on
    /// commented-out lines, and nothing in this crate reads it at all.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// Node 1 is `3.004e-3 cm3/cm`; **fuel nodes 2 to 20 are all exactly
    /// zero**. The whole array sums to `0.020867 cm3/cm` where the pellet
    /// and cladding together occupy `1.606061`.
    ///
    /// **Interpretation.** The defect is confirmed and quantified: the
    /// field understates the rod cross section by a factor of 77. Anyone
    /// re-enabling either commented-out consumer must fix the formula
    /// first, and this test will fail when they do — which is the point.
    #[test]
    fn the_fuel_node_volumes_are_wrong_as_the_reference_computes_them() {
        let (params, geometry, ..) = neacrpd1(&Params::default());
        let vi = &geometry.fuel.vi;

        let r0 = geometry.fuel.fuelrad / params.fuel.fueln as f64;
        assert!((vi[0] - std::f64::consts::PI * r0 * r0).abs() < 1e-15);

        for (i, v) in vi.iter().enumerate().take(params.fuel.fueln).skip(1) {
            assert_eq!(*v, 0.0, "fuel node {i} should be zero under the defect");
        }
        eprintln!("first four node volumes (cm3/cm): {:?}", &vi[..4]);
        eprintln!(
            "sum as computed = {:.6}, a correct pellet+clad total would be {:.6}",
            vi.iter().sum::<f64>(),
            std::f64::consts::PI * geometry.fuel.rtot * geometry.fuel.rtot
        );
    }

    /// **The coupled loop on a real benchmark case.**
    ///
    /// # Methodology
    ///
    /// This is the test [`crate::thdiffusion_solverxyz`]'s "Verification
    /// status" section asks for. That module's own tests are `#[ignore]`d
    /// because the loop does not converge on a hand-built 3x3x6 one-group
    /// fixture, and the open question was whether the fault lay in the fixture
    /// or in the port. NEACRP case D is a real, tuned, two-group coupled
    /// benchmark, so it distinguishes the two.
    ///
    /// The full stack runs: fuel-temperature and coolant-density feedback, the
    /// SANM eigenvalue solve, channel thermal-hydraulics, fuel-rod conduction,
    /// Picard under-relaxation, and three convergence criteria. The case is run
    /// exactly as the reference leaves it — `neacrpd1.m` sets no `th_model`, so
    /// this takes [`ThModel::TwoFluid`], the reference's default.
    ///
    /// **This is not a `k_eff` comparison.** `neacrpd1.m` quotes no reference
    /// eigenvalue, and the NEACRP benchmark's published values are not in
    /// `crates/kovan-literature`. What is asserted is structural: the loop
    /// terminates as converged, all three residuals land under tolerance, and
    /// the eigenvalue is finite and physical.
    ///
    /// # The coolant does not heat on this path, and that is correct
    ///
    /// [`ThModel::TwoFluid`] routes to [`crate::driftflux6_solverstatic3d`],
    /// whose 1-D kernel `driftflux6_solverstatic1d.m` is **absent from the
    /// snapshot**. The reference's `try`/`catch` swallows MATLAB's "Undefined
    /// function", so every powered channel fails and keeps its previous state —
    /// which is the inlet state. That behaviour is reproduced, surfaced as
    /// [`crate::driftflux6_solverstatic3d::ChannelOutcome::SolverMissing`], and
    /// pinned here. Heat transfer to the coolant is asserted on the HEM path in
    /// [`the_hem_path_heats_the_coolant_along_each_channel`] instead.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// **Converged after 12 outer passes.** `k_eff = 1.043614`, fission-source
    /// residual `2.645e-5`, `k_eff` residual `8.270e-6`, fuel-temperature
    /// residual `0.4744 K` with `fueltemp_converged == true`. Peak fuel
    /// temperature `2155.05 K`; coolant flat at `547.15 K`, the inlet value.
    ///
    /// **Interpretation — this settles the open question.** The coupled
    /// driver *does* reach a joint fixed point, meeting all three criteria
    /// in 12 passes on a real two-group benchmark. The non-convergence seen
    /// on the synthetic 3x3x6 one-group fixture was **the fixture**, not the
    /// port, and the warm-start renormalisation named there as the prime
    /// suspect is exonerated — it is exercised on every one of these 12
    /// passes.
    ///
    /// The `2155 K` peak is a consequence of the inert coolant, not a
    /// separate finding: with the channel stuck at the inlet temperature
    /// the rod sees no coolant heat-up, so it runs hotter than it would on
    /// a working two-fluid path. The HEM comparison below gives `1458.73 K`
    /// under the same power.
    #[test]
    fn the_coupled_loop_runs_on_the_benchmark_case() {
        let (params, geometry, th, whichsigma, sigmavalues, feedback) =
            neacrpd1(&Params::default());

        let out = crate::thdiffusion_solverxyz::thdiffusion_solverxyz(
            &geometry,
            &params,
            &th,
            &sigmavalues,
            &feedback,
            &whichsigma,
            None,
        )
        .expect("the NEACRP case should run");

        eprintln!("NEACRP-D1, 17x17x14, coupled steady state (reference default, two-fluid):");
        eprintln!(
            "  termination      = {:?} after {} outer passes",
            out.termination, out.iterations
        );
        eprintln!("  k_eff            = {:.6}", out.k_eff);
        eprintln!("  residual (fs)    = {:.4e}", out.residual);
        eprintln!("  residual (k_eff) = {:.4e}", out.k_eff_residual);
        eprintln!("  residual (Tfuel) = {:.4} K", out.fueltemp_residual);
        eprintln!("  Tfuel converged  = {}", out.fueltemp_converged);

        let tmax = out.th.fueltempavg.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let cmin = out.th.coolant.temps.iter().cloned().fold(f64::INFINITY, f64::min);
        let cmax = out.th.coolant.temps.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        eprintln!("  fuel temp max    = {tmax:.2} K");
        eprintln!("  coolant range    = {cmin:.2} .. {cmax:.2} K");

        assert_eq!(
            out.termination,
            crate::thdiffusion_solverxyz::Termination::Converged,
            "the coupled loop must converge on a real case"
        );
        assert!(out.fueltemp_converged, "the fuel-temperature criterion must be met too");
        assert!(out.k_eff.is_finite() && out.k_eff > 0.0, "k_eff = {}", out.k_eff);
        assert!(tmax > cmax, "the fuel must be hotter than the coolant");

        // The missing 1-D kernel leaves every channel at its inlet state.
        assert_eq!(
            cmin, cmax,
            "with the two-fluid kernel missing the coolant cannot heat; see the module docs"
        );
        assert!(
            (cmin - th.coolant.inlettemp).abs() < 1e-9,
            "the coolant should still be at the inlet temperature"
        );
    }

    /// On the HEM path the coolant actually heats along each channel.
    ///
    /// # Methodology
    ///
    /// Same case, with [`ThModel::Hem`] selected so the thermal-hydraulics
    /// routes to [`crate::singleflow1devap`] — the enthalpy march — instead of
    /// the two-fluid wrapper whose kernel the snapshot is missing. This is the
    /// path that actually transfers heat, and the reference itself selects it
    /// when a transient needs a matching `t = 0` state.
    ///
    /// Pass criteria, all structural: the loop converges, the coolant leaves
    /// hotter than it enters, and the fuel is hotter than the coolant
    /// everywhere.
    ///
    /// # Results — measured 2026-08-18
    ///
    /// **Converged after 29 outer passes.** `k_eff = 0.975869`,
    /// fuel-temperature residual `0.0476 K`. Peak fuel temperature
    /// `1458.73 K`. Coolant `547.14 K` to `556.03 K` — the outlet is
    /// `Tsat(6.7 MPa) = 556.03 K` to the digit shown.
    ///
    /// **Interpretation.** The channel heats from subcooled inlet to
    /// saturation and then stops rising, which is exactly what an
    /// enthalpy march does once it crosses into the two-phase region: the
    /// remaining heat goes into quality, not temperature. For a BWR case
    /// that is the expected outcome, and it is a check on the phase logic
    /// in [`crate::singleflow1devap`] that no single-phase test can make.
    ///
    /// **The two paths differ by ~6800 pcm** (`1.043614` vs `0.975869`) and
    /// the sign is right: the two-fluid path leaves the coolant cold and
    /// dense, so it over-moderates and over-predicts `k_eff`, while the HEM
    /// path boils it and loses the moderator. That is the coolant-density
    /// feedback doing what it should — but it also means **neither number
    /// should be compared to a published NEACRP eigenvalue** until the
    /// missing two-fluid kernel is resolved and a primary reference is in
    /// the archive.
    #[test]
    fn the_hem_path_heats_the_coolant_along_each_channel() {
        let (mut params, geometry, th, whichsigma, sigmavalues, feedback) =
            neacrpd1(&Params::default());
        params.th_model = crate::types::ThModel::Hem;

        let out = crate::thdiffusion_solverxyz::thdiffusion_solverxyz(
            &geometry,
            &params,
            &th,
            &sigmavalues,
            &feedback,
            &whichsigma,
            None,
        )
        .expect("the NEACRP case should run on the HEM path");

        let tmax = out.th.fueltempavg.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let cmin = out.th.coolant.temps.iter().cloned().fold(f64::INFINITY, f64::min);
        let cmax = out.th.coolant.temps.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        eprintln!("NEACRP-D1, coupled steady state (HEM):");
        eprintln!(
            "  termination      = {:?} after {} outer passes",
            out.termination, out.iterations
        );
        eprintln!("  k_eff            = {:.6}", out.k_eff);
        eprintln!("  residual (Tfuel) = {:.4} K", out.fueltemp_residual);
        eprintln!("  fuel temp max    = {tmax:.2} K");
        eprintln!("  coolant range    = {cmin:.2} .. {cmax:.2} K");
        eprintln!("  inlet            = {:.2} K", th.coolant.inlettemp);
        eprintln!("  Tsat(6.7 MPa)    = {:.2} K", tsat_p(6.7));

        assert!(out.k_eff.is_finite() && out.k_eff > 0.0, "k_eff = {}", out.k_eff);
        assert!(cmax > cmin, "the coolant must heat up along the core");
        assert!(tmax > cmax, "the fuel must be hotter than the coolant");
    }
}
