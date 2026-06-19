use csv::Writer;
use uom::si::f64::*;
/// generic enum for a Transfer Function
#[derive(Debug,PartialEq, PartialOrd, Clone)]
pub enum TransferFn {
    FirstOrder(TransferFnFirstOrder),
    SecondOrder(TransferFnSecondOrder),
}

impl Default for TransferFn {
    fn default() -> TransferFn {
        TransferFnFirstOrder::default().into()
    }
}

impl TransferFnTraits for TransferFn {
    fn set_dead_time(&mut self, dead_time: Time) {
        match self {
            TransferFn::FirstOrder(first_order) => {
                first_order.set_dead_time(dead_time)
            },
            TransferFn::SecondOrder(second_order) => {
                second_order.set_dead_time(dead_time)
            },
        }
    }


    fn set_user_input_and_calc(&mut self, 
        user_input: Ratio,
        time_of_input: Time) -> 
    Result<Ratio, ChemEngProcessControlSimulatorError> {
        match self {
            TransferFn::FirstOrder(first_order) => {
                first_order.set_user_input_and_calc(user_input, time_of_input)
            },
            TransferFn::SecondOrder(second_order) => {
                second_order.set_user_input_and_calc(user_input, time_of_input)
            },
        }
    }

    fn spawn_writer(&mut self, name: String) -> Result<Writer<std::fs::File>,
    ChemEngProcessControlSimulatorError>{
        match self {
            TransferFn::FirstOrder(first_order) => {
                first_order.spawn_writer(name)
            },
            TransferFn::SecondOrder(second_order) => {
                second_order.spawn_writer(name)
            },
        }
    }

    fn csv_write_values(&mut self, 
        wtr: &mut Writer<std::fs::File>,
        time: Time,
        input: Ratio,
        output: Ratio) -> Result<(), 
    ChemEngProcessControlSimulatorError> {
        match self {
            TransferFn::FirstOrder(first_order) => {
                first_order.csv_write_values(wtr, time, input, output)
            },
            TransferFn::SecondOrder(second_order) => {
                second_order.csv_write_values(wtr, time,
                    input, output)
            },
        }
    }


}



pub trait TransferFnTraits {
    fn set_dead_time(&mut self, dead_time: Time);
    fn set_user_input_and_calc(&mut self, 
        user_input: Ratio,
        time_of_input: Time) -> Result<Ratio, 
    ChemEngProcessControlSimulatorError>;

    fn spawn_writer(&mut self, name: String) -> Result<Writer<std::fs::File>,
    ChemEngProcessControlSimulatorError>;

    fn csv_write_values(&mut self, 
        wtr: &mut Writer<std::fs::File>,
        time: Time,
        input: Ratio,
        output: Ratio) -> Result<(), 
    ChemEngProcessControlSimulatorError>;
}


pub mod generic_second_order;
pub use generic_second_order::TransferFnSecondOrder;
pub mod generic_first_order;
pub use generic_first_order::TransferFnFirstOrder;

use super::errors::ChemEngProcessControlSimulatorError;



