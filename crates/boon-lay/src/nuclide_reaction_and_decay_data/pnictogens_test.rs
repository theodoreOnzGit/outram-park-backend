use crate::nuclide_reaction_and_decay_data::NuclideReactionAndDecayData;
use crate::decay_xml_info_serde::*;


#[test] 
fn test_nitrogen_parsing(){
    let nitrogen_raw_data: SerdeNuclideVec = nitrogen::get_nitrogen_xml_serde_data();

    let nuclide_vec_raw = nitrogen_raw_data.nuclides;
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
fn test_phosphorous_parsing(){
    let phosphorous_raw_data: SerdeNuclideVec = phosphorous::get_phosphorous_xml_serde_data();

    let nuclide_vec_raw = phosphorous_raw_data.nuclides;
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
fn test_arsenic_parsing(){
    let arsenic_raw_data: SerdeNuclideVec = arsenic::get_arsenic_xml_serde_data();

    let nuclide_vec_raw = arsenic_raw_data.nuclides;
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
fn test_antimony_parsing(){
    let antimony_raw_data: SerdeNuclideVec = antimony::get_antimony_xml_serde_data();

    let nuclide_vec_raw = antimony_raw_data.nuclides;
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
fn test_bismuth_parsing(){
    let bismuth_raw_data: SerdeNuclideVec = bismuth::get_bismuth_xml_serde_data();

    let nuclide_vec_raw = bismuth_raw_data.nuclides;
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


// moscovium not included


