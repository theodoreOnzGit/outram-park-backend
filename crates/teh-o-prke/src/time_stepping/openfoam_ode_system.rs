/// rust translation of the OpenFOAM ODE system
///
/// note that this is nested inside ODESolver struct
#[allow(non_snake_case)]
#[derive(Debug,Clone)]
pub struct ODESystem {
    /// takes in two vectors, x and y 
    /// and then outputs dydx
    user_specified_ode_system: fn(f64, &Vec<f64>) -> Vec<f64>
}



impl ODESystem {

    /// constructor for ODE system 
    /// probably want some easier ways to construct the ode system
    pub fn new(ode_system: fn(f64, &Vec<f64>) -> Vec<f64>) -> Self
    {
        Self {
            user_specified_ode_system: ode_system
        }
    }


    /// this evaluates a vector dydx based on a vector y and 
    /// scalar coordinate x
    pub fn derivatives(&self,
        x: f64,
        y: &Vec<f64>,
        dydx: &mut Vec<f64>) {

        let dydx_local = (self.user_specified_ode_system)(x,y);

        *dydx = dydx_local;


    }

    /// this evaluates a vector dydx based on a vector y and 
    /// scalar coordinate x
    pub fn derivatives_with_fn(
        ode_system: impl Fn(f64, &Vec<f64>) -> Vec<f64>,
        x: f64,
        y: &Vec<f64>,) -> Vec<f64> {

        let dydx = (ode_system)(x,y);

        return dydx;


    }

}
