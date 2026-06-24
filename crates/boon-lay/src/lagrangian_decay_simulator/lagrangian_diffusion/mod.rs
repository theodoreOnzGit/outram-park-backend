/// for lagrangian_diffusion,
/// chatgpt 5 has given a pretty straightforward monte 
/// carlo type diffusion simulator. 
///
/// This is a good simulator to start off
///
///
/// 
#[cfg(test)]
pub mod chatgpt_5_diffusion_simulator;

/// the next thing is to have a library of diffusion coefficients
/// for various particles in silicon carbide, carbon and such
///
/// This will require some literature review.
///
/// (anyway I did some literature review in another repository)
///
///
/// I was also thinking at night of a problem with random walk with diffusion. 
/// The mean free path is horrendously short, and the random walk would 
/// take many collisions in order to get somewhere. 
///
/// How would I know where one would go? 
///
///
/// Thankfully, there is such thing as the central limit theorem.
///
/// Here, we are summing the result of many random walks of isotropically 
/// distributed collisions. This is the mean or sum of many isotropically 
/// distributed collisions. The mean itself or sum  itself must be 
/// normally distributed according to the central limit theorem. In effect,
/// we don't care as much about the individual random walk, as much as the 
/// summation of the random walks.
///
/// For vectors, in the (x,y,z) direction, 
/// https://en.wikipedia.org/wiki/Central_limit_theorem
///
/// We can apply the central limit theorem and have them converge 
/// to a multivariate normal distribution.
///
/// https://en.wikipedia.org/wiki/Multivariate_normal_distribution
///
/// @article{hill2011approximating,
///  title={APPROXIMATING THE RANDOM WALK USING THE CENTRAL LIMIT THEOREM},
///  author={HILL, MITCH},
///  year={2011}
///}
///
///
/// as chatgpt5 mentions:
///
/// For diffusive limits, many small isotropic scatterings with exponential 
/// steps converge to a Gaussian displacement; but the microscopic model 
/// should use the isotropic step sampling above.

#[cfg(test)]
pub mod chatgpt_5_diffusion_normal_dist_central_limit_theorem_simulator;

/// the above was distance based, 
/// Now I want a time based model 
/// because simulator runs on simulated time
/// This is just sample code
#[cfg(test)]
pub mod chatgpt_5_diffusion_normal_dist_central_limit_theorem_simulator_time_based;

/// this is to generate a list of vectors for a spherical shell between r1 and r2
#[cfg(test)]
pub mod chatgpt_5_list_of_vectors_uniformly_distributed_on_spherical_shell;

/// this is to draw 
/// a triso particle widget in egui
#[cfg(test)]
pub mod triso_particle_widget;

/// this module contains functions for Gaussian distributions, 
/// where multiple isotropic scatterings are summed together to 
/// produce a Gaussian distribution due to the central limit theorem 
///
/// this is partly vibe coded from ChatGPT, then edited to fit the needs 
/// of this crate
pub mod central_limit_theorem;

/// contains functions for isotropic scattering 
/// allows particle to finish random walk with isotropic scattering
pub mod isotropic_scattering;

/// this module converts a thermodynamic temperature into a number 
/// of collisions expected on a per unit time basis 
pub mod temperature_dependent_collisions;

/// this is for simulation of a single particle 
/// isotropic material and isotropic scattering (no medium boundaries and 
/// such).
pub mod single_particle_simulator;
