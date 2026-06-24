use crate::prelude::*;


#[test] 
fn test_beryllium_parsing(){
    let beryllium_raw_data: SerdeNuclideVec = beryllium::get_beryllium_xml_serde_data();

    let nuclide_vec_raw = beryllium_raw_data.nuclides;
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
fn test_magnesium_parsing(){
    let magnesium_raw_data: SerdeNuclideVec = magnesium::get_magnesium_xml_serde_data();

    let nuclide_vec_raw = magnesium_raw_data.nuclides;
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
fn test_calcium_parsing(){
    let calcium_raw_data: SerdeNuclideVec = calcium::get_calcium_xml_serde_data();

    let nuclide_vec_raw = calcium_raw_data.nuclides;
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
fn test_strontium_parsing(){
    let strontium_raw_data: SerdeNuclideVec = strontium::get_strontium_xml_serde_data();

    let nuclide_vec_raw = strontium_raw_data.nuclides;
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
fn test_barium_parsing(){
    let barium_raw_data: SerdeNuclideVec = barium::get_barium_xml_serde_data();

    let nuclide_vec_raw = barium_raw_data.nuclides;
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
fn test_radium_parsing(){
    let radium_raw_data: SerdeNuclideVec = radium::get_radium_xml_serde_data();

    let nuclide_vec_raw = radium_raw_data.nuclides;
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
