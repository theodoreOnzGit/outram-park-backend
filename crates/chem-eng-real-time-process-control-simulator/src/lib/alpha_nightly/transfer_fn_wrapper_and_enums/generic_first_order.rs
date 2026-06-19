use csv::Writer;
use uom::si::ratio::ratio;
use uom::si::f64::*;
use uom::si::time::second;
use uom::ConstZero;

use crate::alpha_nightly::errors::ChemEngProcessControlSimulatorError;
use crate::alpha_nightly::stable_transfer_functions::first_order_transfer_fn::FirstOrderStableTransferFnNoZeroes;
use crate::alpha_nightly::stable_transfer_functions::first_order_transfer_fn_with_zeroes::FirstOrderStableTransferFnForZeroes;

use super::{TransferFn, TransferFnTraits};

/// an enum describing generic second order systems,
/// only stable systems are implemented so far
///
/// you are meant to put in:
///
/// G(s) = 
///
/// a1 s + b1 
/// ----------
/// a2 s + b2 
///
/// There are three kinds, 
///
/// 1. Stable 
/// 2. Undamped (constant value)
/// 3. Unstable
///
/// Now this is actually a summation of two first order transfer 
/// functions and some offset
///
/// let Kp = b1/b2
/// let tau_p = a2/b2
///
/// we get:
///
/// G(s) = 
///
///     Kp          a1        /             1          \
/// ----------- + ----------- |  1 -  ---------------- |
/// tau_p s + 1   b2 * taup   \        taup s + 1      /
///
///
/// The first term is taken care of by a 
/// FirstOrderStableTransferFnNoZeroes,
///
/// the second term, is there due to the zeroes, therefore 
/// it is take care of by 
/// FirstOrderStableTransferFnForZeroes
#[derive(Debug,PartialEq, PartialOrd, Clone)]
pub enum TransferFnFirstOrder {
    /// this is arranged in the order
    /// no_zero_transfer_fn,
    /// cosine_term,
    /// sine_term
    Stable(FirstOrderStableTransferFnNoZeroes,
        FirstOrderStableTransferFnForZeroes),
        Unstable,
        ConstantValueUndamped,
}

impl Default for TransferFnFirstOrder {
    fn default() -> Self {
        let zeroes_transferfn = FirstOrderStableTransferFnForZeroes::default();
        let first_order_transfer_fn = FirstOrderStableTransferFnNoZeroes::default();
        TransferFnFirstOrder::Stable(first_order_transfer_fn,zeroes_transferfn)
    }
}

impl TransferFnTraits for TransferFnFirstOrder {
    fn set_dead_time(&mut self, dead_time: Time){

        match self {
            TransferFnFirstOrder::Stable(
                transfer_fn_no_zeroes,
                transfer_fn_for_zeroes) => {
                    // have to test if this works correctly
                    transfer_fn_no_zeroes.delay = dead_time;
                    transfer_fn_for_zeroes.delay = dead_time;

            },
            TransferFnFirstOrder::Unstable => todo!(),
            TransferFnFirstOrder::ConstantValueUndamped => todo!(),
        }
    }


    fn set_user_input_and_calc(&mut self, user_input: Ratio,
        time: Time) -> 
    Result<Ratio, ChemEngProcessControlSimulatorError> {

        match self {
            TransferFnFirstOrder::Stable(
                first_order_no_zeroes,
                first_order_zeroes) => {
                    let mut response: Ratio = Ratio::ZERO;

                    let first_order_output_1 = 
                    first_order_no_zeroes.set_user_input_and_calc_output(
                        time, user_input)?;

                    let first_order_output_2 = 
                    first_order_zeroes.set_user_input_and_calc_output(
                        time, user_input)?;
                    //dbg!(sine_decaying_output);
                    //dbg!(cosine_decaying_output);
                    //dbg!(tf_no_zeroes_output);

                    response += first_order_output_1;
                    response += first_order_output_2;
                    return Ok(response);

                },
            TransferFnFirstOrder::Unstable => todo!(),
            TransferFnFirstOrder::ConstantValueUndamped => todo!(),
        }

    }

    fn spawn_writer(&mut self, name: String) -> Result<Writer<std::fs::File>,
    ChemEngProcessControlSimulatorError>{
        let mut title_string: String = name;
        match self {
            TransferFnFirstOrder::Stable(_,_) => {
                title_string += "1st_ord_transfer_fn_stable.csv";
            },
            TransferFnFirstOrder::Unstable => todo!(),
            TransferFnFirstOrder::ConstantValueUndamped => todo!(),
        }
        let wtr = Writer::from_path(title_string)?;
        Ok(wtr)
    }

    fn csv_write_values(&mut self, 
        wtr: &mut Writer<std::fs::File>,
        time: Time,
        input: Ratio,
        output: Ratio) -> Result<(), 
    ChemEngProcessControlSimulatorError> {

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

impl TransferFnFirstOrder {


    /// generic constructor based on polynomials
    /// This is in the form 
    ///
    /// G(s) = 
    ///
    /// a1 s + b1 
    /// ----------
    /// a2 s + b2 
    ///
    /// 
    pub fn new(a1: Time, 
        b1: Ratio, 
        a2: Time,
        b2: Ratio) -> Result<Self,ChemEngProcessControlSimulatorError> {
        // process time 
        let tau_p: Time = a2/b2;

        // process_gain 
        // I assume units of c1 are dimensionless
        let k_p: Ratio = b1/b2;

        // process gain for zero
        let k_p_for_zero: Ratio = a1/b2/tau_p;

        // now, dead time is zero by default, 
        // you'll have to use a method to manually set dead time

        let tau_p_value: f64 = tau_p.get::<second>();

        if tau_p_value < 0.0 {
            // unstable system
            todo!("unstable system, not implemented");
        } else if tau_p_value == 0.0 {
            // undamped system
            todo!("undamped constant system, not implemented");

        } else {
            // stable system
            let first_ord_transfer_fn_no_zeroes = FirstOrderStableTransferFnNoZeroes::
                new(
                    k_p,
                    tau_p,
                    Ratio::ZERO,
                    Ratio::ZERO,
                    Time::ZERO,
                )?;
            let first_ord_transfer_fn_for_zeroes = FirstOrderStableTransferFnForZeroes::
                new(
                    k_p_for_zero,
                    tau_p,
                    Ratio::ZERO,
                    Ratio::ZERO,
                    Time::ZERO,
                )?;

            return Ok(Self::Stable(
                    first_ord_transfer_fn_no_zeroes, 
                    first_ord_transfer_fn_for_zeroes));
        }


    }



}



impl Into<TransferFn> for TransferFnFirstOrder {
    fn into(self) -> TransferFn {
        TransferFn::FirstOrder(self)
    }
}

impl TryFrom<TransferFn> for TransferFnFirstOrder {
    type Error = ChemEngProcessControlSimulatorError;
    fn try_from(generic_transfer_function: TransferFn) 
    -> Result<Self, Self::Error> {

        if let TransferFn::FirstOrder(
        second_order) = generic_transfer_function {
            return Ok(second_order);
        } else {
            return Err(ChemEngProcessControlSimulatorError::WrongTransferFnType);
        };


    }
}

