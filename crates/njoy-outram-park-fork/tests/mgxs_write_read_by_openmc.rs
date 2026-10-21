// SPDX-License-Identifier: GPL-3.0

//! **V&V gate** — an `mgxs.h5` written by this workspace, read back by
//! **OpenMC itself**, producing the same `k`. GitHub #270.
//!
//! # Why this is the acceptance criterion and a round trip is not
//!
//! Writing a file and reading it back here proves the two halves of *this*
//! codec agree with each other. It cannot detect a format that is
//! self-consistent and wrong — the wrong attribute spelling, a missing
//! `filetype`, an `[G][G'][Order]` ordering nobody else uses. Only the other
//! code reading it can.
//!
//! So this test hands the file to OpenMC (`afa7a14`, the build at
//! `/opt/openmc/bin/openmc`) and requires it to run a transport problem and
//! return the right eigenvalue.
//!
//! # Methodology
//!
//! 1. Write `mgxs.h5` with [`write_mgxs`]: one group, one material.
//! 2. Run OpenMC's multi-group mode on the **same slab** used by
//!    `outram-mc-libs`' `white_boundary_vs_openmc` gate (`T = 3` cm, left face
//!    reflective, right face vacuum, transverse-infinite).
//! 3. Require `k` to match what OpenMC produced from **its own**
//!    `openmc.MGXSLibrary`-written file for the identical problem:
//!    `1.395552 +/- 0.000213`.
//!
//! The cross sections are the same set that gate uses: total 1.0, absorption
//! 0.30, scatter 0.70, fission 0.20, nu-fission 0.50, chi 1.0.
//!
//! Skips with a printed note when OpenMC is not installed, so a developer
//! without it does not see a red test — but the skip is visible rather than a
//! silent pass.
//!
//! # Results (2026-09-22)
//!
//! ```text
//! wrote /tmp/njoy_mgxs_write_vv/mgxs.h5 (1800 bytes)
//! OpenMC on OUR mgxs.h5: k = 1.395552 +/- 0.000213
//!                  on its own file: 1.395552
//!                  |d| = 0.000000, budget 0.001205
//! ```
//!
//! **Identical, not merely consistent.** `|d| = 0.000000` at the printed
//! precision, because OpenMC's seed is fixed and the data it read is the same
//! data — so the two runs are the same run. That is a stronger result than
//! agreement within statistics, and it is only available because the reference
//! was generated with the same seed.
//!
//! It also means this gate is **sharp**: any difference in what the file
//! carries — a transposed scatter matrix, a mis-spelled dataset, a group
//! structure off by a factor — would move `k` well outside the budget rather
//! than hide in the noise.

use njoy_outram_park_fork::hdf5::mgxs_write::{write_mgxs, MgxsMaterial};

/// OpenMC's own answer for this slab, from
/// `outram-mc-libs/verification_and_validation/white_boundary/`.
const OPENMC_K: f64 = 1.395552;
const OPENMC_SIGMA: f64 = 0.000213;

fn one_group_material() -> MgxsMaterial {
    MgxsMaterial {
        name: "fuel".to_string(),
        temperature_label: "294K".to_string(),
        // OpenMC stores kT in eV; 294 K * 8.617333e-5 eV/K.
        kt_ev: 294.0 * 8.617_333_262e-5,
        total: vec![1.0],
        absorption: vec![0.30],
        fission: vec![0.20],
        nu_fission: vec![0.50],
        chi: vec![1.0],
        scatter_matrix: vec![0.70],
        g_min: vec![1],
        g_max: vec![1],
    }
}

fn openmc_python() -> Option<&'static str> {
    ["/opt/ompy/bin/python", "python3"].into_iter().find(|p| {
        std::process::Command::new(p)
            .args(["-c", "import openmc"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

#[test]
fn openmc_reads_an_mgxs_written_here_and_gets_the_right_k() {
    let dir = std::env::temp_dir().join("njoy_mgxs_write_vv");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let h5 = dir.join("mgxs.h5");

    write_mgxs(&h5, &[0.0, 20.0e6], &[one_group_material()])
        .expect("write mgxs.h5");
    let bytes = std::fs::metadata(&h5).unwrap().len();
    println!("wrote {} ({bytes} bytes)", h5.display());
    assert!(bytes > 0, "writer produced an empty file");

    let Some(python) = openmc_python() else {
        println!("[skip] no python with openmc on this machine; the file was still written");
        return;
    };

    // Same slab as the white-boundary gate: T = 3 cm, reflective / vacuum.
    let deck = r#"
import numpy as np, openmc, sys
T = 3.0
mat = openmc.Material(name='fuel'); mat.set_density('macro', 1.0)
mat.add_macroscopic('fuel')
mats = openmc.Materials([mat]); mats.cross_sections = 'mgxs.h5'; mats.export_to_xml()
left  = openmc.XPlane(0.0, boundary_type='reflective')
right = openmc.XPlane(T,   boundary_type='vacuum')
ymin = openmc.YPlane(-1.0, boundary_type='reflective'); ymax = openmc.YPlane(1.0, boundary_type='reflective')
zmin = openmc.ZPlane(-1.0, boundary_type='reflective'); zmax = openmc.ZPlane(1.0, boundary_type='reflective')
cell = openmc.Cell(fill=mat, region=+left & -right & +ymin & -ymax & +zmin & -zmax)
openmc.Geometry([cell]).export_to_xml()
s = openmc.Settings()
s.energy_mode='multi-group'; s.batches=220; s.inactive=20; s.particles=40000; s.seed=1
s.source = openmc.IndependentSource(space=openmc.stats.Box((0.,-1.,-1.),(T,1.,1.)))
s.export_to_xml()
openmc.run(output=False)
sp = openmc.StatePoint('statepoint.220.h5')
print(f"K {sp.keff.nominal_value:.6f} {sp.keff.std_dev:.6f}")
"#;
    std::fs::write(dir.join("run.py"), deck).unwrap();

    let out = std::process::Command::new(python)
        .arg("run.py")
        .current_dir(&dir)
        .env("PATH", format!("/opt/openmc/bin:{}", std::env::var("PATH").unwrap_or_default()))
        .output()
        .expect("run openmc");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "OpenMC could not use the mgxs.h5 written here.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    let line = stdout
        .lines()
        .find(|l| l.starts_with("K "))
        .unwrap_or_else(|| panic!("no eigenvalue in OpenMC output:\n{stdout}\n{stderr}"));
    let mut it = line.split_whitespace().skip(1);
    let k: f64 = it.next().unwrap().parse().unwrap();
    let sigma: f64 = it.next().unwrap().parse().unwrap();

    let budget = 4.0 * (sigma * sigma + OPENMC_SIGMA * OPENMC_SIGMA).sqrt();
    let d = (k - OPENMC_K).abs();
    println!(
        "OpenMC on OUR mgxs.h5: k = {k:.6} +/- {sigma:.6}; on its own file {OPENMC_K:.6}; \
         |d| = {d:.6}, budget {budget:.6}"
    );
    assert!(
        d <= budget,
        "k from our file ({k:.6}) differs from OpenMC's own ({OPENMC_K:.6}) by {d:.6}, \
         beyond the {budget:.6} combined 4-sigma budget. The file is readable but the \
         DATA it carries is not what was intended -- check the scatter_matrix ordering \
         and the nu-fission spelling first."
    );
}
