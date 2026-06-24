/// this basically allows the user to select the open panel
#[derive(serde::Deserialize, serde::Serialize,PartialEq,Clone)]
pub(crate) enum Panel {
    MainPage,
    GraphPage,
    PeriodicTable,
}


pub mod citation_disclaimer_and_acknowledgements;

pub mod main_page;

pub mod graph_page;

pub mod side_panel;

/// this provides a legend as to what elements are what colour
pub mod periodic_table;
