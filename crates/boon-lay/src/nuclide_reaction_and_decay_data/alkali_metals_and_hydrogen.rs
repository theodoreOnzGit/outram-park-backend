use crate::nuclide_reaction_and_decay_data::NuclideReactionAndDecayData;
use crate::decay_xml_info_serde::*;


#[test] 
fn test_hydrogen_parsing(){
    let hydrogen_raw_data: SerdeNuclideVec = hydrogen::get_hydrogen_xml_serde_data();

    let nuclide_vec_raw = hydrogen_raw_data.nuclides;
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
fn test_lithium_parsing(){
    let lithium_raw_data: SerdeNuclideVec = lithium::get_lithium_xml_serde_data();

    let nuclide_vec_raw = lithium_raw_data.nuclides;
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
fn test_sodium_parsing(){
    let sodium_raw_data: SerdeNuclideVec = sodium::get_sodium_xml_serde_data();

    let nuclide_vec_raw = sodium_raw_data.nuclides;
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
fn test_potassium_parsing(){
    let potassium_raw_data: SerdeNuclideVec = potassium::get_potassium_xml_serde_data();

    let nuclide_vec_raw = potassium_raw_data.nuclides;
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
fn test_rubidium_parsing(){
    let rubidium_raw_data: SerdeNuclideVec = rubidium::get_rubidium_xml_serde_data();

    let nuclide_vec_raw = rubidium_raw_data.nuclides;
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
fn test_cesium_parsing(){
    let cesium_raw_data: SerdeNuclideVec = cesium::get_cesium_xml_serde_data();

    let nuclide_vec_raw = cesium_raw_data.nuclides;
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
fn test_francium_parsing(){
    let francium_raw_data: SerdeNuclideVec = francium::get_francium_xml_serde_data();

    let nuclide_vec_raw = francium_raw_data.nuclides;
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
