use crate::nuclide_reaction_and_decay_data::NuclideReactionAndDecayData;
use crate::decay_xml_info_serde::*;
#[test]
fn test_actinides_parsing() {
    // Assumes each actinide has a module with a function named get_<element>_xml_serde_data()
    // analogous to zinc::get_zinc_xml_serde_data().
    // If your crate uses different naming (e.g., symbols), let me know and I can adjust.

    let all_raw_data: Vec<SerdeNuclideVec> = vec![
        actinium::get_actinium_xml_serde_data(),
        thorium::get_thorium_xml_serde_data(),
        protactinium::get_protactinium_xml_serde_data(),
        uranium::get_uranium_xml_serde_data(),
        neptunium::get_neptunium_xml_serde_data(),
        plutonium::get_plutonium_xml_serde_data(),
        americium::get_americium_xml_serde_data(),
        curium::get_curium_xml_serde_data(),
        berkelium::get_berkelium_xml_serde_data(),
        californium::get_californium_xml_serde_data(),
        einsteinium::get_einsteinium_xml_serde_data(),
        fermium::get_fermium_xml_serde_data(),
        mendelevium::get_mendelevium_xml_serde_data(),
        nobelium::get_nobelium_xml_serde_data(),
        lawrencium::get_lawrencium_xml_serde_data(),
    ];

    let mut nuclide_vec_processed: Vec<NuclideReactionAndDecayData> = vec![];

    for element_raw_data in all_raw_data {
        let nuclide_vec_raw = element_raw_data.nuclides;

        for raw_nuclide_data in nuclide_vec_raw {
            let nuclide_data: NuclideReactionAndDecayData = raw_nuclide_data
                .try_into()
                .expect("Failed to convert raw nuclide data into processed type");

            // If needed, normalize nuclear isomer naming (e.g., "m1" -> "m") here.
            // e.g., nuclide_data = normalize_isomer_labels(nuclide_data);

            dbg!(&nuclide_data);
            nuclide_vec_processed.push(nuclide_data);
        }
    }

    // Optionally assert expectations about processed data
    // assert!(!nuclide_vec_processed.is_empty());
}
