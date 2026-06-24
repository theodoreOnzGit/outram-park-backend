use crate::nuclide_reaction_and_decay_data::NuclideReactionAndDecayData;
use crate::decay_xml_info_serde::*;


#[test] 
fn test_oxygen_parsing(){
    let oxygen_raw_data: SerdeNuclideVec = oxygen::get_oxygen_xml_serde_data();

    let nuclide_vec_raw = oxygen_raw_data.nuclides;
    let mut nuclide_vec_processed: Vec<NuclideReactionAndDecayData>
        = vec![];

    for raw_nuclide_data in nuclide_vec_raw {

        let nuclide_data: NuclideReactionAndDecayData 
            = raw_nuclide_data.try_into().unwrap();
        // now, in doing this test, I realise the nuclear isomers have 
        // different naming conventions
        //
        // for example m1 is meant by m in my crate
        //
        // the openmc part m1, needs to be replaced by m

        dbg!(&nuclide_data);
        nuclide_vec_processed.push(nuclide_data);
        
    }
}


#[test] 
fn test_sulfur_parsing(){
    let sulfur_raw_data: SerdeNuclideVec = sulfur::get_sulfur_xml_serde_data();

    let nuclide_vec_raw = sulfur_raw_data.nuclides;
    let mut nuclide_vec_processed: Vec<NuclideReactionAndDecayData>
        = vec![];

    for raw_nuclide_data in nuclide_vec_raw {

        let nuclide_data: NuclideReactionAndDecayData 
            = raw_nuclide_data.try_into().unwrap();
        // now, in doing this test, I realise the nuclear isomers have 
        // different naming conventions
        //
        // for example m1 is meant by m in my crate
        //
        // the openmc part m1, needs to be replaced by m

        dbg!(&nuclide_data);
        nuclide_vec_processed.push(nuclide_data);
        
    }
}

#[test] 
fn test_selenium_parsing(){
    let selenium_raw_data: SerdeNuclideVec = selenium::get_selenium_xml_serde_data();

    let nuclide_vec_raw = selenium_raw_data.nuclides;
    let mut nuclide_vec_processed: Vec<NuclideReactionAndDecayData>
        = vec![];

    for raw_nuclide_data in nuclide_vec_raw {

        let nuclide_data: NuclideReactionAndDecayData 
            = raw_nuclide_data.try_into().unwrap();
        // now, in doing this test, I realise the nuclear isomers have 
        // different naming conventions
        //
        // for example m1 is meant by m in my crate
        //
        // the openmc part m1, needs to be replaced by m

        dbg!(&nuclide_data);
        nuclide_vec_processed.push(nuclide_data);
        
    }
}

#[test] 
fn test_tellurium_parsing(){
    let tellurium_raw_data: SerdeNuclideVec = tellurium::get_tellurium_xml_serde_data();

    let nuclide_vec_raw = tellurium_raw_data.nuclides;
    let mut nuclide_vec_processed: Vec<NuclideReactionAndDecayData>
        = vec![];

    for raw_nuclide_data in nuclide_vec_raw {

        let nuclide_data: NuclideReactionAndDecayData 
            = raw_nuclide_data.try_into().unwrap();
        // now, in doing this test, I realise the nuclear isomers have 
        // different naming conventions
        //
        // for example m1 is meant by m in my crate
        //
        // the openmc part m1, needs to be replaced by m

        dbg!(&nuclide_data);
        nuclide_vec_processed.push(nuclide_data);
        
    }
}
#[test] 
fn test_polonium_parsing(){
    let polonium_raw_data: SerdeNuclideVec = polonium::get_polonium_xml_serde_data();

    let nuclide_vec_raw = polonium_raw_data.nuclides;
    let mut nuclide_vec_processed: Vec<NuclideReactionAndDecayData>
        = vec![];

    for raw_nuclide_data in nuclide_vec_raw {

        let nuclide_data: NuclideReactionAndDecayData 
            = raw_nuclide_data.try_into().unwrap();
        // now, in doing this test, I realise the nuclear isomers have 
        // different naming conventions
        //
        // for example m1 is meant by m in my crate
        //
        // the openmc part m1, needs to be replaced by m

        dbg!(&nuclide_data);
        nuclide_vec_processed.push(nuclide_data);
        
    }
}


// livermorium not included

