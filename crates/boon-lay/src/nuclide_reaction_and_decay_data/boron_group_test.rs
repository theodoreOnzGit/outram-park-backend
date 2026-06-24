use crate::nuclide_reaction_and_decay_data::NuclideReactionAndDecayData;
use crate::decay_xml_info_serde::*;
// vibe coded
#[test]
fn test_boron_group_parsing() {
    // Assumes each element exposes get_<element>_xml_serde_data(), analogous to zinc::get_zinc_xml_serde_data().
    // Adjust module/function names if your crate differs (e.g., aluminum vs aluminium).
    let all_raw_data: Vec<SerdeNuclideVec> = vec![
        boron::get_boron_xml_serde_data(),
        aluminium::get_aluminium_xml_serde_data(), // If your crate uses "aluminum", rename accordingly.
        gallium::get_gallium_xml_serde_data(),
        indium::get_indium_xml_serde_data(),
        thallium::get_thallium_xml_serde_data(),
        // If your dataset includes superheavy elements:
        // nihonium::get_nihonium_xml_serde_data(),
    ];

    let mut nuclide_vec_processed: Vec<NuclideReactionAndDecayData> = vec![];

    for element_raw_data in all_raw_data {
        let nuclide_vec_raw = element_raw_data.nuclides;

        for raw_nuclide_data in nuclide_vec_raw {
            let nuclide_data: NuclideReactionAndDecayData = raw_nuclide_data
                .try_into()
                .expect("Failed to convert raw nuclide data into processed type");

            // If needed, normalize nuclear isomer naming (e.g., "m1" -> "m") here.

            dbg!(&nuclide_data);
            nuclide_vec_processed.push(nuclide_data);
        }
    }

    // Optionally: assertions about nuclide_vec_processed
    // assert!(!nuclide_vec_processed.is_empty());
}
