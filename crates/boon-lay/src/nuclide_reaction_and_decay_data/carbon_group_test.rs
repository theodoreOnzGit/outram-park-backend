use crate::nuclide_reaction_and_decay_data::NuclideReactionAndDecayData;
use crate::decay_xml_info_serde::*;
// vibe coded
#[test]
fn test_carbon_group_parsing() {
    // Assumes each element exposes get_<element>_xml_serde_data(), analogous to zinc::get_zinc_xml_serde_data().
    // Adjust module/function names if your crate differs.
    let all_raw_data: Vec<SerdeNuclideVec> = vec![
        carbon::get_carbon_xml_serde_data(),
        silicon::get_silicon_xml_serde_data(),
        germanium::get_germanium_xml_serde_data(),
        tin::get_tin_xml_serde_data(),
        lead::get_lead_xml_serde_data(),
        // If your dataset includes superheavy elements:
        // flerovium::get_flerovium_xml_serde_data(),
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


