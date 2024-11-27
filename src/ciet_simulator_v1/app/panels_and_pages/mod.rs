
#[derive(serde::Deserialize, serde::Serialize,PartialEq,Clone)]
pub(crate) enum Panel {
    SchematicDiagram,
    MainPage,
    CTAHPump,
    CTAH,
    Heater,
    DHX,
    TCHX,
}

pub mod main_page;

pub mod heater_page;

pub mod ctah_page;

pub mod ciet_data;

/// contains code for natural circulation only
pub mod nat_circ_simulation;
