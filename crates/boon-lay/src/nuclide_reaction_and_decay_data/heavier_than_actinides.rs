use crate::nuclide_reaction_and_decay_data::NuclideReactionAndDecayData;
use crate::decay_xml_info_serde::*;
#[test]
fn test_rutherfordium_to_roentgenium_parsing() {
    // Assumes each superheavy element exposes get_<element>_xml_serde_data(),
    // analogous to zinc::get_zinc_xml_serde_data(). Adjust names if your crate differs.
    let all_raw_data: Vec<SerdeNuclideVec> = vec![
        rutherfordium::get_rutherfordium_xml_serde_data(), // Rf (104)
        dubnium::get_dubnium_xml_serde_data(),             // Db (105)
        seaborgium::get_seaborgium_xml_serde_data(),       // Sg (106)
        bohrium::get_bohrium_xml_serde_data(),             // Bh (107)
        hassium::get_hassium_xml_serde_data(),             // Hs (108)
        meitnerium::get_meitnerium_xml_serde_data(),       // Mt (109)
        darmstadtium::get_darmstadtium_xml_serde_data(),   // Ds (110)
        roentgenium::get_roentgenium_xml_serde_data(),     // Rg (111)
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
