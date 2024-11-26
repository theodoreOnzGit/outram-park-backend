
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
