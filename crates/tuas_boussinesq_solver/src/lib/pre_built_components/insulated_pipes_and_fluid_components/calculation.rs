use super::InsulatedFluidComponent;
use uom::si::f64::*;
use crate::tuas_lib_error::TuasLibError;
use std::thread::JoinHandle;
use std::thread;

impl InsulatedFluidComponent {

    /// advances the timestep for each HeatTransferEntity within this
    /// InsulatedFluidComponent (pipe fluid array, pipe shell and
    /// insulation), updating each control-volume array's temperatures.
    ///
    /// `timestep` is the time increment in seconds.
    #[inline]
    pub fn advance_timestep(&mut self, 
    timestep: Time) -> Result<(),TuasLibError> {

        self.pipe_fluid_array.advance_timestep_mut_self(timestep)?;
        self.pipe_shell.advance_timestep_mut_self(timestep)?;
        self.insulation.advance_timestep_mut_self(timestep)?;
        Ok(())
        
    }


    /// advances the timestep by cloning this component, moving the clone
    /// into a spawned thread that runs `advance_timestep`, and returning the
    /// `JoinHandle`. Unwrapping the handle yields the advanced component.
    ///
    /// This lets several components advance in parallel; `timestep` is the
    /// time increment in seconds.
    pub fn advance_timestep_thread_spawn(&self,
        timestep: Time,) -> JoinHandle<Self> {

        // make a clone
        let mut fluid_component_clone = self.clone();

        // move ptr into a new thread 

        let join_handle = thread::spawn(
            move || -> Self {


                // carry out the connection calculations
                fluid_component_clone.advance_timestep(timestep).unwrap();
                
                fluid_component_clone

            }
        );

        return join_handle;

    }
}
