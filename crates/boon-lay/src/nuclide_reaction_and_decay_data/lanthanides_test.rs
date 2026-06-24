use crate::nuclide_reaction_and_decay_data::NuclideReactionAndDecayData;
use crate::decay_xml_info_serde::*;
#[test]
fn test_lanthanides_parsing() {
    // Assumes each lanthanide has a module with a function named get_<element>_xml_serde_data(),
    // analogous to zinc::get_zinc_xml_serde_data(). Adjust names if your crate differs.
    let all_raw_data: Vec<SerdeNuclideVec> = vec![
        lanthanum::get_lanthanum_xml_serde_data(),
        cerium::get_cerium_xml_serde_data(),
        praseodymium::get_praseodymium_xml_serde_data(),
        neodymium::get_neodymium_xml_serde_data(),
        promethium::get_promethium_xml_serde_data(),
        samarium::get_samarium_xml_serde_data(),
        europium::get_europium_xml_serde_data(),
        gadolinium::get_gadolinium_xml_serde_data(),
        terbium::get_terbium_xml_serde_data(),
        dysprosium::get_dysprosium_xml_serde_data(),
        holmium::get_holmium_xml_serde_data(),
        erbium::get_erbium_xml_serde_data(),
        thulium::get_thulium_xml_serde_data(),
        ytterbium::get_ytterbium_xml_serde_data(),
        lutetium::get_lutetium_xml_serde_data(),
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
