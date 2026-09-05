//! Pre-built components for the University of Wisconsin-Madison FLiBe
//! (molten fluoride salt, LiF-BeF2) forced/natural-circulation loop.
//!
//! The physical loop and its instrumentation are described in:
//! Britsch, K., Anderson, M., Brooks, P., & Sridharan, K. (2019).
//! Natural circulation FLiBe loop overview. International Journal of
//! Heat and Mass Transfer, 134, 970-983.
//!
//! This module is organised as two successive modelling iterations of the
//! same 13-component loop (all pipes are 1 inch / 2.54 cm outer diameter with
//! a 3 mm wall, so the inner diameter is 2.54 cm - 2*3 mm; FLiBe is the working
//! fluid). Component lengths are in metres, incline angles in degrees:
//!
//! - [`flibe_loop_iteration_one`] — first-cut loop: adiabatic pipe components,
//!   parasitic-heat-loss calibration against the reference test tables, and
//!   the fluid-mechanics / thermal-hydraulics calculation routines that link
//!   the components into a single circulating branch and advance it in time.
//! - [`flibe_loop_iteration_two`] — adds an explicit clamshell radiative
//!   heater sub-component to capture the heat added (and lost) by the loop's
//!   clamshell radiative heating elements, which iteration one could not
//!   represent.
//!
//! Both iteration modules are currently gated behind `#[cfg(test)]` — they are
//! used from the loop's verification/simulation tests rather than the public
//! library surface.

#[cfg(test)]
/// First iteration of UW madison flibe loop given the following
/// best estimate parameters.
///
/// [Component no.],[description],[length (m)],[angle],[Delta x],[Delta y],
/// [1],[hot leg insulated heater vertical],[1.47],[90],[9.00115397373305E-17],[1.47],
/// [2],[opening to tank],[0.12],[0],[0.12],[0],
/// [3],[top left bend vertical],[0.0897],[-90],[5.49254089417588E-18],[-0.0897],
/// [4],[top left bend diagonal],[0.0897],[-58.9],[0.0463330395993378],[-0.0768071574886494],
/// [5],[cold leg horizontal],[1.35],[-10],[1.32949046656648],[-0.234425039850356],
/// [6],[Cold leg corner bend],[0.0897],[-20],[0.084290428084496],[-0.0306792068563125],
/// [7],[cold leg vertical],[1.53],[-90],[9.36854801347725E-17],[-1.53],
/// [8],[cold to hot leg bend 1],[0.0598],[-100],[-0.0103841610244824],[-0.05889150363013],
/// [9],[cold to hot leg bend 2],[0.0598],[-150],[-0.0517883191463094],[-0.0299],
/// [10],[cold to hot leg bend 3],[0.0598],[180],[-0.0598],[7.32338785890117E-18],
/// [11],[hot leg horizontal-ish],[1.42],[160],[-1.33436352151599],[0.48566860352245],
/// [12],[hot leg bend 1],[0.0897],[130],[-0.0576580485888826],[0.0687141865477723],
/// [13],[hot leg bend 2],[0.0697],[158],[-0.0646247146633051],[0.0261100795610891],
pub mod flibe_loop_iteration_one;

#[cfg(test)]
/// Second iteration of UW madison flibe loop
///
///
/// compared to the first iteration, iteration number two includes
/// the use and simulation of radiative heat transfer heaters
///
/// in component 2, the temperature of the fluid exiting pipe 2
/// tends to be increased comapred to the entrance temperature.
///
/// This implies that there is heat added in this component.
/// Why?
///
/// One possible explanation is of radiative heat transfer as the
/// heaters of the flibe loop are clamshell radiative heat transfer
/// heaters. Therefore, it is inevitable that some of the heat escapes
/// into the surrounding.
///
/// Britsch, K., Anderson, M., Brooks, P., & Sridharan, K. (2019).
/// Natural circulation FLiBe loop overview. International Journal
/// of Heat and Mass Transfer, 134, 970-983.
///
/// Watlow Cera- mic Fiber Semi-Cylindrical heaters has a hot face length of 61 cm
/// (24 in.) and an internal diameter of 5 cm (2 in.). The embedded ele-
/// ments provide proportionally-controlled power up-to 1.7 kW, on a
/// 208 V circuit. A 2.54 cm long vestibule caps each heater to center it
/// on the tube and limit convection losses. The heater exterior is
/// wrapped in three layers of Pyrogel HPS insulation, which reduces
/// surface temperatures to fairly constant 300 °C
///
pub mod flibe_loop_iteration_two;
