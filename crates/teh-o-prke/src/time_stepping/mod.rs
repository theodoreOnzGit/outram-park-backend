// for Rodas solvers:
//
//     Hairer, E., Nørsett, S. P., & Wanner, G. (1996).
//    Solving Ordinary Differential Equations II:
//    Stiff and Differential-Algebraic Problems, second edition",
//    Springer-Verlag, Berlin.
//
// for Rosenbrock solvers:
//
//     Verwer, J. G., Spee, E. J., Blom, J. G., & Hundsdorfer, W. (1999).
//    A second-order Rosenbrock method applied to
//    photochemical dispersion problems.
//    SIAM Journal on Scientific Computing, 20(4), 1456-1480.

#[allow(non_upper_case_globals)]
pub mod openfoam_rfk45;

pub mod openfoam_ode_system;

pub mod openfoam_ode_solver;
