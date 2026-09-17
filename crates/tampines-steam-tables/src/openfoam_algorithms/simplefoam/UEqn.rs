//! Reference-only, entirely commented-out C++ source mirroring OpenFOAM
//! `simpleFoam`'s `UEqn.H` — the momentum predictor: assembles
//! `fvm::div(phi, U) + MRF.DDt(U) + turbulence->divDevReff(U) == fvOptions(U)`,
//! relaxes it, and (if enabled) solves `UEqn == -fvc::grad(p)` for velocity.
//! Not compiled or ported to Rust.
//
//    // Momentum predictor
//
//    MRF.correctBoundaryVelocity(U);
//
//    tmp<fvVectorMatrix> tUEqn
//    (
//        fvm::div(phi, U)
//      + MRF.DDt(U)
//      + turbulence->divDevReff(U)
//     ==
//        fvOptions(U)
//    );
//    fvVectorMatrix& UEqn = tUEqn.ref();
//
//    UEqn.relax();
//
//    fvOptions.constrain(UEqn);
//
//    if (simple.momentumPredictor())
//    {
//        solve(UEqn == -fvc::grad(p));
//
//        fvOptions.correct(U);
//    }
