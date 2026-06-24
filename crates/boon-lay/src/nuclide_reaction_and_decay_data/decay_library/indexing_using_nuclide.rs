use fission_yields_data::prelude::Nuclide;

use crate::prelude::NuclideReactionAndDecayData;

use super::DecayLibrary;

impl DecayLibrary {
    #[inline]
    pub fn try_match_nuclides_to_decay_data(&self, nuclide: Nuclide) 
        -> Option<NuclideReactionAndDecayData> {

            // once decay library is loaded, I first get z a of nuclide 

            let (proton_number,_a) = nuclide.get_z_a();

            // i will then match the proton number 

            let decay_data_vec_for_element: Vec<NuclideReactionAndDecayData> = 
                match proton_number {

                    1 => self.hydrogen_data.clone(),
                    2 => self.helium_data.clone(),
                    3 => self.lithium_data.clone(),
                    4 => self.beryllium_data.clone(),
                    5 => self.boron_data.clone(),
                    6 => self.carbon_data.clone(),
                    7 => self.nitrogen_data.clone(),
                    8 => self.oxygen_data.clone(),
                    9 => self.fluorine_data.clone(),
                    10 => self.neon_data.clone(),
                    11 => self.sodium_data.clone(),
                    12 => self.magnesium_data.clone(),
                    13 => self.aluminium_data.clone(),
                    14 => self.silicon_data.clone(),
                    15 => self.phosphorus_data.clone(),
                    16 => self.sulfur_data.clone(),
                    17 => self.chlorine_data.clone(),
                    18 => self.argon_data.clone(),
                    19 => self.potassium_data.clone(),
                    20 => self.calcium_data.clone(),
                    21 => self.scandium_data.clone(),
                    22 => self.titanium_data.clone(),
                    23 => self.vanadium_data.clone(),
                    24 => self.chromium_data.clone(),
                    25 => self.manganese_data.clone(),
                    26 => self.iron_data.clone(),
                    27 => self.cobalt_data.clone(),
                    28 => self.nickel_data.clone(),
                    29 => self.copper_data.clone(),
                    30 => self.zinc_data.clone(),
                    31 => self.gallium_data.clone(),
                    32 => self.germanium_data.clone(),
                    33 => self.arsenic_data.clone(),
                    34 => self.selenium_data.clone(),
                    35 => self.bromine_data.clone(),
                    36 => self.krypton_data.clone(),
                    37 => self.rubidium_data.clone(),
                    38 => self.strontium_data.clone(),
                    39 => self.yttrium_data.clone(),
                    40 => self.zirconium_data.clone(),
                    41 => self.niobium_data.clone(),
                    42 => self.molybdenum_data.clone(),
                    43 => self.technetium_data.clone(),
                    44 => self.ruthenium_data.clone(),
                    45 => self.rhodium_data.clone(),
                    46 => self.palladium_data.clone(),
                    47 => self.silver_data.clone(),
                    48 => self.cadmium_data.clone(),
                    49 => self.indium_data.clone(),
                    50 => self.tin_data.clone(),
                    51 => self.antimony_data.clone(),
                    52 => self.tellurium_data.clone(),
                    53 => self.iodine_data.clone(),
                    54 => self.xenon_data.clone(),
                    55 => self.cesium_data.clone(),
                    56 => self.barium_data.clone(),
                    57 => self.lanthanum_data.clone(),
                    58 => self.cerium_data.clone(),
                    59 => self.praseodymium_data.clone(),
                    60 => self.neodymium_data.clone(),
                    61 => self.promethium_data.clone(),
                    62 => self.samarium_data.clone(),
                    63 => self.europium_data.clone(),
                    64 => self.gadolinium_data.clone(),
                    65 => self.terbium_data.clone(),
                    66 => self.dysprosium_data.clone(),
                    67 => self.holmium_data.clone(),
                    68 => self.erbium_data.clone(),
                    69 => self.thulium_data.clone(),
                    70 => self.ytterbium_data.clone(),
                    71 => self.lutetium_data.clone(),
                    72 => self.hafnium_data.clone(),
                    73 => self.tantalum_data.clone(),
                    74 => self.tungsten_data.clone(),
                    75 => self.rhenium_data.clone(),
                    76 => self.osmium_data.clone(),
                    77 => self.iridium_data.clone(),
                    78 => self.platinum_data.clone(),
                    79 => self.gold_data.clone(),
                    80 => self.mercury_data.clone(),
                    81 => self.thallium_data.clone(),
                    82 => self.lead_data.clone(),
                    83 => self.bismuth_data.clone(),
                    84 => self.polonium_data.clone(),
                    85 => self.astatine_data.clone(),
                    86 => self.radon_data.clone(),
                    87 => self.francium_data.clone(),
                    88 => self.radium_data.clone(),
                    89 => self.actinium_data.clone(),
                    90 => self.thorium_data.clone(),
                    91 => self.protactinium_data.clone(),
                    92 => self.uranium_data.clone(),
                    93 => self.neptunium_data.clone(),
                    94 => self.plutonium_data.clone(),
                    95 => self.americium_data.clone(),
                    96 => self.curium_data.clone(),
                    97 => self.berkelium_data.clone(),
                    98 => self.californium_data.clone(),
                    99 => self.einsteinium_data.clone(),
                    100 => self.fermium_data.clone(),
                    101 => self.mendelevium_data.clone(),
                    102 => self.nobelium_data.clone(),
                    103 => self.lawrencium_data.clone(),
                    104 => self.rutherfordium_data.clone(),
                    105 => self.dubnium_data.clone(),
                    106 => self.seaborgium_data.clone(),
                    107 => self.bohrium_data.clone(),
                    108 => self.hassium_data.clone(),
                    109 => self.meitnerium_data.clone(),
                    110 => self.darmstadtium_data.clone(),
                    111 => self.roentgenium_data.clone(),
                    112 => todo!("copernicium::get_copernicium_xml_serde_data() not available yet"),
                    113 => todo!("nihonium::get_nihonium_xml_serde_data() not available yet"),
                    114 => todo!("flerovium::get_flerovium_xml_serde_data() not available yet"),
                    115 => todo!("moscovium::get_moscovium_xml_serde_data() not available yet"),
                    116 => todo!("livermorium::get_livermorium_xml_serde_data() not available yet"),
                    117 => todo!("tennessine::get_tennessine_xml_serde_data() not available yet"),
                    118 => todo!("oganesson::get_oganesson_xml_serde_data() not available yet"),
                    _ => todo!("proton number supplied is invalid or 0 (neutron)"),
                };

            // from this I need to index and match the nuclide to its 
            // decay information

            let mut nuclide_reaction_and_decay_data_result_vec: 
                Vec<NuclideReactionAndDecayData>
                = vec![];

            for nuclide_decay_data in decay_data_vec_for_element {

                if nuclide == nuclide_decay_data.nuclide {

                    nuclide_reaction_and_decay_data_result_vec 
                        .push(nuclide_decay_data);

                }

            }

            // if size is not equal 1, error 

            if nuclide_reaction_and_decay_data_result_vec.len() != 1 {
                return None;
            }
            

            let nuclide_data = nuclide_reaction_and_decay_data_result_vec[0].clone();

            return Some(nuclide_data);
    }
}
