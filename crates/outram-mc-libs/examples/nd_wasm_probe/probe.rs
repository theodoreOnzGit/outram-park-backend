//! The probe's work, as plain functions: called by the wasm exports in
//! `lib.rs` (in headless Chromium) and by `tests/nd_wasm_probe_native.rs`
//! (natively), so both runs execute the same code on the same bytes and
//! print fingerprints that must be equal. See
//! `verification_and_validation/nuclear_data_loading_web/README.md`.

use njoy_outram_park_fork::acer::ace_set::AceSet;
use njoy_outram_park_fork::archive::write_zip;
use njoy_outram_park_fork::endf_library::{generate_ace, AceRequest, EndfLibraryIndex};
use njoy_outram_park_fork::units::Temperature as ProcessTemperature;
use outram_mc_libs::model::prelude::*;
use outram_mc_libs::vv::godiva;
use uom::si::f64::{Length, ThermodynamicTemperature};
use uom::si::length::centimeter;
use uom::si::thermodynamic_temperature::kelvin;

/// FNV-1a over bytes, continuing from `h` (`0xcbf2_9ce4_8422_2325` to start).
pub fn fnv(bytes: &[u8], mut h: u64) -> u64 {
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// The FNV offset basis.
pub const FNV0: u64 = 0xcbf2_9ce4_8422_2325;

/// **Zip the files, then index the zip**: `(zip bytes, report lines)`. The
/// report holds the zip's fingerprint, the index's fingerprint (every tape's
/// path, ZA, LISO, MAT, NSUB, AWR bits and library), the main library, the
/// layout, the counts and the picker's first hit for "u235".
pub fn zip_and_index(files: &[(String, Vec<u8>)]) -> Result<(Vec<u8>, String), String> {
    let zip = write_zip(files).map_err(|e| e.to_string())?;
    let ix = EndfLibraryIndex::from_zip_bytes(zip.clone()).map_err(|e| e.to_string())?;
    let mut h = FNV0;
    for t in ix.tapes() {
        let x = &t.header;
        h = fnv(t.path.as_bytes(), h);
        h = fnv(&x.za.to_le_bytes(), h);
        h = fnv(&[x.liso], h);
        h = fnv(&x.mat.unwrap_or(-1).to_le_bytes(), h);
        h = fnv(&x.nsub.unwrap_or(-1).to_le_bytes(), h);
        h = fnv(&x.awr.to_bits().to_le_bytes(), h);
        h = fnv(x.library().as_bytes(), h);
    }
    let main = ix.main_library().unwrap_or_default();
    let first = ix
        .search(&main, "u235")
        .first()
        .map(|t| t.path.clone())
        .unwrap_or_default();
    let report = format!(
        "zip_bytes={}\nzip_fnv={:#018x}\nindex_fnv={h:#018x}\nmain_library={main}\nlayout={:?}\ntapes={}\nskipped={}\nsearch_u235={first}\n",
        zip.len(),
        fnv(&zip, FNV0),
        ix.layout(),
        ix.tapes().len(),
        ix.skipped().len()
    );
    Ok((zip, report))
}

/// **Generate F-19's ACE at 293.6 K with its 0 K companion** from the
/// zipped library (#936's browser generation), and read them back by header.
pub fn generate_f19(zip: &[u8]) -> Result<String, String> {
    let ix = EndfLibraryIndex::from_zip_bytes(zip.to_vec()).map_err(|e| e.to_string())?;
    let main = ix.main_library().ok_or("no library")?;
    let f19 = ix
        .incident_neutron(&main, 9, 19, 0)
        .map_err(|e| e.to_string())?;
    let out = generate_ace(
        &ix,
        f19,
        &AceRequest::new(ProcessTemperature::new::<kelvin>(293.6)),
    )
    .map_err(|e| e.to_string())?;
    let mut h = FNV0;
    // The ACE header's `hk` comment carries the build's git commit; the
    // fingerprint skips the first two lines (the header) of each file.
    for g in &out {
        h = fnv(g.path.as_bytes(), h);
        let body: Vec<&str> = std::str::from_utf8(&g.bytes)
            .map_err(|e| e.to_string())?
            .lines()
            .skip(2)
            .collect();
        h = fnv(body.join("\n").as_bytes(), h);
    }
    let set = AceSet::from_files(
        out.iter()
            .map(|g| (g.path.clone(), g.bytes.clone()))
            .collect(),
    );
    let hot = set.neutron(9019, 293.6).map_err(|e| e.to_string())?;
    let cold = set.zero_kelvin(9019).map_err(|e| e.to_string())?;
    Ok(format!(
        "ace_files={}\nace_fnv={h:#018x}\nace_hot={} {}\nace_cold={} {}\n",
        out.len(),
        hot.zaid(),
        hot.file,
        cold.zaid(),
        cold.file
    ))
}

/// **Godiva from the zipped library**: `NuclearData::from_zip_bytes`, the
/// beginner model, 200 neutrons × [2 + 3] cycles at seed 5. Reports the
/// fingerprint of every cycle's `k` (its bits) and the mean.
pub fn godiva_from_zip(zip: &[u8]) -> Result<String, String> {
    let data = NuclearData::from_zip_bytes("probe.zip", zip.to_vec()).map_err(|e| e.to_string())?;
    let mut m = Model::new(data);
    let core = m.component("Core");
    let heu = Material::from_atom_densities(
        godiva::NUCLIDES
            .iter()
            .map(|(n, _, d)| (Isotope::from_name(n).unwrap_or(U235), atoms_per_barn_cm(*d))),
    );
    m.add(
        Sphere::radius(Length::new::<centimeter>(godiva::RADIUS_CM))
            .filled_with(heu)
            .in_component(core),
    );
    m.set_temperature(Temperature::Uniform(
        ThermodynamicTemperature::new::<kelvin>(godiva::TEMPERATURE_K),
    ));
    let run = m
        .keff(KeffSettings::new(
            MCSettings::Custom(CustomRun {
                particles: 200,
                inactive: 2,
                active: 3,
                seed: 5,
            }),
            None,
        ))
        .map_err(|e| e.to_string())?;
    let mut h = FNV0;
    for k in &run.k_by_cycle {
        h = fnv(&k.to_bits().to_le_bytes(), h);
    }
    Ok(format!(
        "godiva_k_fnv={h:#018x}\ngodiva_k={:.15}\ngodiva_cycles={}\n",
        run.keff().mean,
        run.k_by_cycle.len()
    ))
}

/// **ACE files indexed and decoded as a nuclide** (the ACE way in): the
/// fingerprint of every table's XSS words, and U-234 built from its table.
pub fn ace_tables(files: Vec<(String, Vec<u8>)>) -> Result<String, String> {
    let set = AceSet::from_files(files.clone());
    let mut h = FNV0;
    for t in set.tables() {
        h = fnv(t.zaid().as_bytes(), h);
        for x in &t.table.xss {
            h = fnv(&x.to_bits().to_le_bytes(), h);
        }
    }
    let data = NuclearData::from_ace_files("probe ACE", files).map_err(|e| e.to_string())?;
    let n = data
        .load_nuclide(U234, 293.6, None)
        .map_err(|e| e.to_string())?;
    Ok(format!(
        "ace_tables={}\nace_xss_fnv={h:#018x}\nu234_dbrc={}\n",
        set.tables().len(),
        n.has_dbrc()
    ))
}
