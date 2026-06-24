use fission_yields_data::prelude::Nuclide;
use crate::{decay_xml_info_serde::*, prelude::decay_library::DecayLibrary};

use super::NuclideReactionAndDecayData;

impl NuclideReactionAndDecayData {

    /// this is a computationally expensive way to obtain decay data 
    /// it basically loads a decay library, and uses the nuclide to index 
    /// the appropriate decay information
    ///
    /// it is, however, here so that it is more convenient syntax wise 
    /// so I leave it to you what you want to use
    #[inline]
    pub fn computationally_expensive_parse_nuclide_to_decay_data(
        nuclide: Nuclide,
    ) -> Option<NuclideReactionAndDecayData>{
            let decay_library = DecayLibrary::new();

            return decay_library.try_match_nuclides_to_decay_data(nuclide);

    }
    
    /// will parse nuclides to obtain decay information 
    /// unless it is a neutron
    #[inline]
    pub fn parse_nuclides_to_decay_data_vec_by_element(nuclide: &Nuclide) 
        -> Vec<NuclideReactionAndDecayData> {

            // first get (z,a) 
            let (proton_number,_a) = nuclide.get_z_a();

            let nuclide_vec_raw: SerdeNuclideVec = match proton_number {
                1 => hydrogen::get_hydrogen_xml_serde_data(),
                2 => helium::get_helium_xml_serde_data(),
                3 => lithium::get_lithium_xml_serde_data(),
                4 => beryllium::get_beryllium_xml_serde_data(),
                5 => boron::get_boron_xml_serde_data(),
                6 => carbon::get_carbon_xml_serde_data(),
                7 => nitrogen::get_nitrogen_xml_serde_data(),
                8 => oxygen::get_oxygen_xml_serde_data(),
                9 => fluorine::get_fluorine_xml_serde_data(),
                10 => neon::get_neon_xml_serde_data(),
                11 => sodium::get_sodium_xml_serde_data(),
                12 => magnesium::get_magnesium_xml_serde_data(),
                13 => aluminium::get_aluminium_xml_serde_data(),
                14 => silicon::get_silicon_xml_serde_data(),
                15 => phosphorous::get_phosphorous_xml_serde_data(),
                16 => sulfur::get_sulfur_xml_serde_data(),
                17 => chlorine::get_chlorine_xml_serde_data(),
                18 => argon::get_argon_xml_serde_data(),
                19 => potassium::get_potassium_xml_serde_data(),
                20 => calcium::get_calcium_xml_serde_data(),
                21 => scandium::get_scandium_xml_serde_data(),
                22 => titanium::get_titanium_xml_serde_data(),
                23 => vanadium::get_vanadium_xml_serde_data(),
                24 => chromium::get_chromium_xml_serde_data(),
                25 => manganese::get_manganese_xml_serde_data(),
                26 => iron::get_iron_xml_serde_data(),
                27 => cobalt::get_cobalt_xml_serde_data(),
                28 => nickel::get_nickel_xml_serde_data(),
                29 => copper::get_copper_xml_serde_data(),
                30 => zinc::get_zinc_xml_serde_data(),
                31 => gallium::get_gallium_xml_serde_data(),
                32 => germanium::get_germanium_xml_serde_data(),
                33 => arsenic::get_arsenic_xml_serde_data(),
                34 => selenium::get_selenium_xml_serde_data(),
                35 => bromine::get_bromine_xml_serde_data(),
                36 => krypton::get_krypton_xml_serde_data(),
                37 => rubidium::get_rubidium_xml_serde_data(),
                38 => strontium::get_strontium_xml_serde_data(),
                39 => yttrium::get_yttrium_xml_serde_data(),
                40 => zirconium::get_zirconium_xml_serde_data(),
                41 => niobium::get_niobium_xml_serde_data(),
                42 => molybdenum::get_molybdenum_xml_serde_data(),
                43 => technetium::get_technetium_xml_serde_data(),
                44 => ruthenium::get_ruthenium_xml_serde_data(),
                45 => rhodium::get_rhodium_xml_serde_data(),
                46 => palladium::get_palladium_xml_serde_data(),
                47 => silver::get_silver_xml_serde_data(),
                48 => cadmium::get_cadmium_xml_serde_data(),
                49 => indium::get_indium_xml_serde_data(),
                50 => tin::get_tin_xml_serde_data(),
                51 => antimony::get_antimony_xml_serde_data(),
                52 => tellurium::get_tellurium_xml_serde_data(),
                53 => iodine::get_iodine_xml_serde_data(),
                54 => xenon::get_xenon_xml_serde_data(),
                55 => cesium::get_cesium_xml_serde_data(),
                56 => barium::get_barium_xml_serde_data(),
                57 => lanthanum::get_lanthanum_xml_serde_data(),
                58 => cerium::get_cerium_xml_serde_data(),
                59 => praseodymium::get_praseodymium_xml_serde_data(),
                60 => neodymium::get_neodymium_xml_serde_data(),
                61 => promethium::get_promethium_xml_serde_data(),
                62 => samarium::get_samarium_xml_serde_data(),
                63 => europium::get_europium_xml_serde_data(),
                64 => gadolinium::get_gadolinium_xml_serde_data(),
                65 => terbium::get_terbium_xml_serde_data(),
                66 => dysprosium::get_dysprosium_xml_serde_data(),
                67 => holmium::get_holmium_xml_serde_data(),
                68 => erbium::get_erbium_xml_serde_data(),
                69 => thulium::get_thulium_xml_serde_data(),
                70 => ytterbium::get_ytterbium_xml_serde_data(),
                71 => lutetium::get_lutetium_xml_serde_data(),
                72 => hafnium::get_hafnium_xml_serde_data(),
                73 => tantalum::get_tantalum_xml_serde_data(),
                74 => tungsten::get_tungsten_xml_serde_data(),
                75 => rhenium::get_rhenium_xml_serde_data(),
                76 => osmium::get_osmium_xml_serde_data(),
                77 => iridium::get_iridium_xml_serde_data(),
                78 => platinum::get_platinum_xml_serde_data(),
                79 => gold::get_gold_xml_serde_data(),
                80 => mercury::get_mercury_xml_serde_data(),
                81 => thallium::get_thallium_xml_serde_data(),
                82 => lead::get_lead_xml_serde_data(),
                83 => bismuth::get_bismuth_xml_serde_data(),
                84 => polonium::get_polonium_xml_serde_data(),
                85 => astatine::get_astatine_xml_serde_data(),
                86 => radon::get_radon_xml_serde_data(),
                87 => francium::get_francium_xml_serde_data(),
                88 => radium::get_radium_xml_serde_data(),
                89 => actinium::get_actinium_xml_serde_data(),
                90 => thorium::get_thorium_xml_serde_data(),
                91 => protactinium::get_protactinium_xml_serde_data(),
                92 => uranium::get_uranium_xml_serde_data(),
                93 => neptunium::get_neptunium_xml_serde_data(),
                94 => plutonium::get_plutonium_xml_serde_data(),
                95 => americium::get_americium_xml_serde_data(),
                96 => curium::get_curium_xml_serde_data(),
                97 => berkelium::get_berkelium_xml_serde_data(),
                98 => californium::get_californium_xml_serde_data(),
                99 => einsteinium::get_einsteinium_xml_serde_data(),
                100 => fermium::get_fermium_xml_serde_data(),
                101 => mendelevium::get_mendelevium_xml_serde_data(),
                102 => nobelium::get_nobelium_xml_serde_data(),
                103 => lawrencium::get_lawrencium_xml_serde_data(),
                104 => rutherfordium::get_rutherfordium_xml_serde_data(),
                105 => dubnium::get_dubnium_xml_serde_data(),
                106 => seaborgium::get_seaborgium_xml_serde_data(),
                107 => bohrium::get_bohrium_xml_serde_data(),
                108 => hassium::get_hassium_xml_serde_data(),
                109 => meitnerium::get_meitnerium_xml_serde_data(),
                110 => darmstadtium::get_darmstadtium_xml_serde_data(),
                111 => roentgenium::get_roentgenium_xml_serde_data(),
                112 => todo!("copernicium::get_copernicium_xml_serde_data() not available yet"),
                113 => todo!("nihonium::get_nihonium_xml_serde_data() not available yet"),
                114 => todo!("flerovium::get_flerovium_xml_serde_data() not available yet"),
                115 => todo!("moscovium::get_moscovium_xml_serde_data() not available yet"),
                116 => todo!("livermorium::get_livermorium_xml_serde_data() not available yet"),
                117 => todo!("tennessine::get_tennessine_xml_serde_data() not available yet"),
                118 => todo!("oganesson::get_oganesson_xml_serde_data() not available yet"),
                _ => todo!("proton number supplied is invalid or 0 (neutron)"),
            };

            let mut nuclide_vec_processed: Vec<NuclideReactionAndDecayData>
                = vec![];

            for raw_nuclide_data in nuclide_vec_raw.nuclides {

                let nuclide_data: NuclideReactionAndDecayData 
                    = raw_nuclide_data.try_into().unwrap();
                // now, in doing this test, I realise the nuclear isomers have 
                // different naming conventions
                //
                // for example m1 is meant by m in my crate
                //
                // the openmc part m1, needs to be replaced by m

                nuclide_vec_processed.push(nuclide_data);

            }

            return nuclide_vec_processed;

    }
}
