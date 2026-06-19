use csv::Writer;
use uom::si::ratio::ratio;
use uom::si::time::second;
use uom::si::f64::*;

pub use self::integral_controller::IntegralController;
pub use self::proportional_controller::ProportionalController;
pub use self::filtered_derivative_controller::FilteredDerivativeController;

use super::errors::ChemEngProcessControlSimulatorError;
use super::transfer_fn_wrapper_and_enums::TransferFnTraits;
pub(crate) mod proportional_controller;
pub mod integral_controller;
pub(crate) mod filtered_derivative_controller;

/// generic enum for a Continuous Time Controller
#[derive(Debug,PartialEq, PartialOrd, Clone)]
pub enum AnalogController {
    PIDFiltered(ProportionalController,IntegralController,FilteredDerivativeController),
    PI(ProportionalController,IntegralController),
    P(ProportionalController),
    PDFiltered(ProportionalController, FilteredDerivativeController),
    IntegralStandalone(IntegralController),
    DerivativeFilteredStandalone(FilteredDerivativeController),
}

impl AnalogController {
    pub fn new_pi_controller(controller_gain: Ratio,
        integral_time: Time) -> Result<Self,ChemEngProcessControlSimulatorError> {

        let p_controller = ProportionalController::new(controller_gain)?;
        let i_controller = IntegralController::new(controller_gain,
            integral_time)?;

        Ok(Self::PI(p_controller, i_controller))
    }

    pub fn new_filtered_pid_controller(controller_gain: Ratio,
        integral_time: Time,
        derivative_time: Time,
        alpha: Ratio,
    ) -> Result<Self,ChemEngProcessControlSimulatorError> {

        let p_controller = ProportionalController::new(controller_gain)?;
        let i_controller = IntegralController::new(controller_gain,
            integral_time)?;
        let d_controller = FilteredDerivativeController::new(
            controller_gain, derivative_time, alpha)?;

        Ok(Self::PIDFiltered(p_controller, i_controller, d_controller))
    }
    pub fn new_filtered_pd_controller(controller_gain: Ratio,
        derivative_time: Time,
        alpha: Ratio,
    ) -> Result<Self,ChemEngProcessControlSimulatorError> {

        let p_controller = ProportionalController::new(controller_gain)?;
        let d_controller = FilteredDerivativeController::new(
            controller_gain, derivative_time, alpha)?;

        Ok(Self::PDFiltered(p_controller, d_controller))
    }
}

impl TransferFnTraits for AnalogController {
    fn set_dead_time(&mut self, dead_time: uom::si::f64::Time) {
        match self {
            AnalogController::PIDFiltered(proportional_controller,
                integral_controller, filtered_derivative_controller) => {
                    proportional_controller.set_dead_time(dead_time);
                    integral_controller.set_dead_time(dead_time);
                    filtered_derivative_controller.set_dead_time(dead_time)
                },
            AnalogController::PI(p_controller, integral_controller) => {
                p_controller.set_dead_time(dead_time);
                integral_controller.set_dead_time(dead_time)
            },
            AnalogController::P(ctrl) => {
                ctrl.set_dead_time(dead_time)
            },
            AnalogController::IntegralStandalone(ctrl) => {
                ctrl.set_dead_time(dead_time)
            },
            AnalogController::DerivativeFilteredStandalone(ctrl) => {
                ctrl.set_dead_time(dead_time)
            },
            AnalogController::PDFiltered(proportional_controller, 
                filtered_derivative_controller) => {
                proportional_controller.set_dead_time(dead_time);
                filtered_derivative_controller.set_dead_time(dead_time)

            },

        }
    }

    fn set_user_input_and_calc(&mut self, 
        user_input: uom::si::f64::Ratio,
        time_of_input: uom::si::f64::Time) -> Result<uom::si::f64::Ratio, 
    super::errors::ChemEngProcessControlSimulatorError> {
        match self {
            AnalogController::PIDFiltered(proportional_controller,
                integral_controller, filtered_derivative_controller) => {
                    let p_output = 
                    proportional_controller.set_user_input_and_calc(user_input, time_of_input)?;
                    let i_output = 
                    integral_controller.set_user_input_and_calc(user_input, time_of_input)?;
                    let d_output = 
                    filtered_derivative_controller.set_user_input_and_calc(user_input, time_of_input)?;

                    return Ok(p_output + i_output + d_output);
                },
            AnalogController::PI(proportional_controller, integral_controller) => {
                let p_output = 
                proportional_controller.set_user_input_and_calc(user_input, time_of_input)?;
                let i_output = 
                integral_controller.set_user_input_and_calc(user_input, time_of_input)?;
                return Ok(p_output + i_output);
            },
            AnalogController::P(ctrl) => {
                ctrl.set_user_input_and_calc(user_input, time_of_input)
            },
            AnalogController::IntegralStandalone(ctrl) => {
                ctrl.set_user_input_and_calc(user_input, time_of_input)
            },
            AnalogController::DerivativeFilteredStandalone(ctrl) => {
                ctrl.set_user_input_and_calc(user_input, time_of_input)
            },
            AnalogController::PDFiltered(proportional_controller, 
                filtered_derivative_controller) => {
                    let p_output = 
                    proportional_controller.set_user_input_and_calc(user_input, time_of_input)?;
                    let d_output = 
                    filtered_derivative_controller.set_user_input_and_calc(user_input, time_of_input)?;

                    return Ok(p_output + d_output);
                },

        }
    }

    fn spawn_writer(&mut self, name: String) -> Result<csv::Writer<std::fs::File>,
    super::errors::ChemEngProcessControlSimulatorError> {
        let mut title_string: String = name;
        match self {
            AnalogController::PIDFiltered(_, _, _) => {
                    title_string += "_PID_controller.csv"

                },
            AnalogController::PI(_, _) => {
                title_string += "_PI_controller.csv"
            },
            AnalogController::P(ctrl) => {
                return ctrl.spawn_writer(title_string);
            },
            AnalogController::IntegralStandalone(ctrl) => {
                return ctrl.spawn_writer(title_string);
            },
            AnalogController::DerivativeFilteredStandalone(ctrl) => {
                return ctrl.spawn_writer(title_string);
            },
            AnalogController::PDFiltered(_proportional_controller, 
                _filtered_derivative_controller) => {
                title_string += "_PD_controller.csv"
            },
        }
        let wtr = Writer::from_path(title_string)?;
        Ok(wtr)
    }

    fn csv_write_values(&mut self, 
        wtr: &mut csv::Writer<std::fs::File>,
        time: uom::si::f64::Time,
        input: uom::si::f64::Ratio,
        output: uom::si::f64::Ratio) -> Result<(), 
    super::errors::ChemEngProcessControlSimulatorError> {
        let current_time_string = time.get::<second>().to_string();
        let input_string = input.get::<ratio>().to_string();
        let output_string = output.get::<ratio>().to_string();

        wtr.write_record(&[current_time_string,
            input_string,
            output_string])?;

        wtr.flush().unwrap();
        Ok(())
    }
}
