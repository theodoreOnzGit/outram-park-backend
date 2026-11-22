/*---------------------------------------------------------------------------*\
  gh:#1363 row 3a: upstream rhoPimpleFoam (OpenFOAM-v2506, ESI) on a 1-D
  shock tube with h-form energy, as a dump driver.

  This is `applications/solvers/compressible/rhoPimpleFoam/rhoPimpleFoam.C`
  with createFields.H, UEqn.H, EEqn.H and pEqn.H (the non-transonic,
  non-consistent branch) transcribed line for line, on the UNMODIFIED
  upstream libraries, including the real fluidThermo::New (hePsiThermo, const
  transport, hConst, perfectGas, sensibleEnthalpy) and the real
  compressible::turbulenceModel::New (laminar). Substitutions, all of them:

    1. fvOptions, MRF, mesh motion (dynamicFvMesh, CorrectPhi, rhoUf, LTS),
       pressureControl (no fixed-value pressure patch, so it never limits) and
       `fvc::makeRelative` are omitted: the case has none of them, and
       libfvOptions / libdynamicFvMesh / libthermoTools are not part of the
       minimal closure.
    2. `runTime.write()` is replaced by hexadecimal dumps.
    3. The continuity errors are also printed as bit patterns.
    4. The laminar model is `Stokes` with mu = 0.

  Output lines as in sonicDump.C.
\*---------------------------------------------------------------------------*/
#include "fvCFD.H"
#include "fluidThermo.H"
#include "turbulentFluidThermoModel.H"
#include "pimpleControl.H"
#include "IOdictionary.H"
#include <cstring>
#include <cstdio>
#include <cstdint>

using namespace Foam;

// rhoEqn.H with fvOptions(rho) (an empty source here) removed
static void solveRhoEqn(volScalarField& rho, const surfaceScalarField& phi)
{
    fvScalarMatrix rhoEqn
    (
        fvm::ddt(rho)
      + fvc::div(phi)
    );

    rhoEqn.solve();
}

static bool dumpStagesNow = false;
static label stepNow = 0;

static void hexOne(const char* tag, const char* name, label i, double v)
{
    std::uint64_t b;
    std::memcpy(&b, &v, sizeof(b));
    std::printf
    (
        "S %d %s %s %d %016llx\n", int(stepNow), tag, name, int(i),
        (unsigned long long)b
    );
}

static void dumpScalar(const char* tag, const char* name, const scalarField& f)
{
    for (label i = 0; i < f.size(); ++i) hexOne(tag, name, i, f[i]);
}

static void dumpVolS(const char* tag, const volScalarField& f)
{
    dumpScalar(tag, f.name().c_str(), f.primitiveField());
}

static void dumpVolUx(const char* tag, const volVectorField& f)
{
    scalarField ux(f.primitiveField().component(0));
    dumpScalar(tag, "Ux", ux);
}

static void dumpMatrix(const char* tag, const fvScalarMatrix& m)
{
    dumpScalar(tag, "diag", m.diag());
    dumpScalar(tag, "source", m.source());
    dumpScalar(tag, "lower", m.lower());
    dumpScalar(tag, "upper", m.upper());
}

static void dumpMatrixSym(const char* tag, const fvScalarMatrix& m)
{
    dumpScalar(tag, "diag", m.diag());
    dumpScalar(tag, "source", m.source());
    dumpScalar(tag, "lower", m.lower());
    dumpScalar(tag, "upper", m.upper());
}

static void dumpMatrixU(const char* tag, const fvVectorMatrix& m)
{
    dumpScalar(tag, "diag", m.diag());
    dumpScalar(tag, "source", scalarField(m.source().component(0)));
    dumpScalar(tag, "lower", m.lower());
    dumpScalar(tag, "upper", m.upper());
}

static void dumpSurf(const char* tag, const surfaceScalarField& f)
{
    dumpScalar(tag, f.name().c_str(), f.primitiveField());
}

static void hexC(const char* name, double v)
{
    std::uint64_t b;
    std::memcpy(&b, &v, sizeof(b));
    std::printf("C %d %s %016llx\n", int(stepNow), name, (unsigned long long)b);
}

int main(int argc, char *argv[])
{
    #include "setRootCase.H"
    #include "createTime.H"
    #include "createMesh.H"
    #include "createControl.H"

    IOdictionary ctrl
    (
        IOobject
        (
            "controlDict", runTime.system(), mesh,
            IOobject::MUST_READ, IOobject::NO_WRITE
        )
    );
    const labelList dumpSteps(ctrl.lookup("dumpSteps"));
    const bool dumpStages = ctrl.lookupOrDefault<bool>("dumpStages", false);
    const label dumpStageSteps = ctrl.lookupOrDefault<label>("dumpStageSteps", 2);
    const label dumpStageFrom = ctrl.lookupOrDefault<label>("dumpStageFrom", 1);

    {
        std::printf("# GEOMETRY\n");
        dumpScalar("geom", "V", mesh.V().field());
        scalarField sfx(mesh.Sf().primitiveField().component(0));
        dumpScalar("geom", "Sfx", sfx);
        dumpScalar("geom", "magSf", mesh.magSf().primitiveField());
        dumpScalar("geom", "w", mesh.weights().primitiveField());
        dumpScalar("geom", "deltaCoeffs", mesh.deltaCoeffs().primitiveField());
        dumpScalar("geom", "nonOrthDeltaCoeffs", mesh.nonOrthDeltaCoeffs().primitiveField());
    }

    // ---- createFields.H
    Info<< "Reading thermophysical properties\n" << endl;
    autoPtr<fluidThermo> pThermo(fluidThermo::New(mesh));
    fluidThermo& thermo = pThermo();
    thermo.validate(args.executable(), "h", "e");

    volScalarField& p = thermo.p();

    volScalarField rho
    (
        IOobject
        (
            "rho",
            runTime.timeName(),
            mesh,
            IOobject::NO_READ,
            IOobject::AUTO_WRITE
        ),
        thermo.rho()
    );

    Info<< "Reading field U\n" << endl;
    volVectorField U
    (
        IOobject
        (
            "U",
            runTime.timeName(),
            mesh,
            IOobject::MUST_READ,
            IOobject::AUTO_WRITE
        ),
        mesh
    );

    #include "compressibleCreatePhi.H"
    autoPtr<surfaceVectorField> rhoUf;

    mesh.setFluxRequired(p.name());

    Info<< "Creating turbulence model\n" << endl;
    autoPtr<compressible::turbulenceModel> turbulence
    (
        compressible::turbulenceModel::New(rho, U, phi, thermo)
    );

    volScalarField dpdt
    (
        IOobject("dpdt", runTime.timeName(), mesh),
        fvc::ddt(p)
    );

    #include "createK.H"

    const dimensionedScalar rhoMax("rhoMax", dimDensity, GREAT, pimple.dict());
    const dimensionedScalar rhoMin("rhoMin", dimDensity, Zero, pimple.dict());

    // ---- createFieldRefs.H
    volScalarField& e = thermo.he();   // "h"
    const volScalarField& psi = thermo.psi();

    #include "initContinuityErrs.H"

    turbulence->validate();

    Info<< "\nStarting time loop\n" << endl;

    while (runTime.loop())
    {
        stepNow = runTime.timeIndex();
        dumpStagesNow = dumpStages && stepNow >= dumpStageFrom && stepNow <= dumpStageSteps;
        Info<< "Time = " << runTime.timeName() << nl << endl;

        #include "compressibleCourantNo.H"

        while (pimple.loop())
        {
            if (pimple.firstIter())
            {
                solveRhoEqn(rho, phi);
                if (dumpStagesNow) dumpVolS("rhoEqn", rho);
            }

            // ---- UEqn.H
            fvVectorMatrix UEqn
            (
                fvm::ddt(rho, U) + fvm::div(phi, U)
              + turbulence->divDevRhoReff(U)
            );

            UEqn.relax();
            if (dumpStagesNow) dumpMatrixU("UEqn:m", UEqn);

            if (pimple.momentumPredictor())
            {
                solve(UEqn == -fvc::grad(p));
                K = 0.5*magSqr(U);
            }
            if (dumpStagesNow) dumpVolUx("UEqn", U);

            // ---- EEqn.H (he is h)
            {
                fvScalarMatrix EEqn
                (
                    fvm::ddt(rho, e) + fvm::div(phi, e)
                  + fvc::ddt(rho, K) + fvc::div(phi, K)
                  - dpdt
                  - fvm::laplacian(turbulence->alphaEff(), e)
                );

                EEqn.relax();
                if (dumpStagesNow) dumpMatrix("EEqn:m", EEqn);
                EEqn.solve();
                thermo.correct();
            }
            if (dumpStagesNow)
            {
                dumpVolS("EEqn", e);
                dumpVolS("EEqn", thermo.T());
            }

            while (pimple.correct())
            {
                // ---- pEqn.H
                rho = thermo.rho();

                const volScalarField psip0(psi*p);

                volScalarField rAU(1.0/UEqn.A());
                surfaceScalarField rhorAUf("rhorAUf", fvc::interpolate(rho*rAU));
                volVectorField HbyA("HbyA", U);
                HbyA = rAU*UEqn.H();

                surfaceScalarField phiHbyA
                (
                    "phiHbyA",
                    fvc::interpolate(rho)*fvc::flux(HbyA)
                  + rhorAUf*fvc::ddtCorr(rho, U, phi, rhoUf)
                );

                if (dumpStagesNow)
                {
                    dumpVolS("pEqn:rAU", rAU);
                    dumpVolUx("pEqn:HbyA", HbyA);
                    dumpSurf("pEqn:phiHbyA", phiHbyA);
                    dumpSurf("pEqn:rhorAUf", rhorAUf);
                }

                fvScalarMatrix pDDtEqn
                (
                    fvc::ddt(rho) + psi*correction(fvm::ddt(p))
                  + fvc::div(phiHbyA)
                );

                while (pimple.correctNonOrthogonal())
                {
                    fvScalarMatrix pEqn(pDDtEqn - fvm::laplacian(rhorAUf, p));

                    if (dumpStagesNow) dumpMatrixSym("pEqn:m", pEqn);
                    pEqn.solve();

                    if (pimple.finalNonOrthogonalIter())
                    {
                        phi = phiHbyA + pEqn.flux();
                    }
                }

                p.relax();

                U = HbyA - rAU*fvc::grad(p);
                U.correctBoundaryConditions();
                K = 0.5*magSqr(U);

                thermo.correctRho(psi*p - psip0, rhoMin, rhoMax);

                solveRhoEqn(rho, phi);
                #include "compressibleContinuityErrs.H"
                {
                    dimensionedScalar totalMass = fvc::domainIntegrate(rho);
                    hexC
                    (
                        "sumLocal",
                        (fvc::domainIntegrate(mag(rho - thermo.rho()))/totalMass).value()
                    );
                    hexC
                    (
                        "global",
                        (fvc::domainIntegrate(rho - thermo.rho())/totalMass).value()
                    );
                }

                rho = thermo.rho();

                if (thermo.dpdt())
                {
                    dpdt = fvc::ddt(p);
                }

                if (dumpStagesNow)
                {
                    dumpVolS("pEqn", p);
                    dumpSurf("pEqn", phi);
                    dumpVolUx("pEqn", U);
                }
            }

            if (pimple.turbCorr())
            {
                turbulence->correct();
            }
        }

        rho = thermo.rho();

        bool dump = false;
        forAll(dumpSteps, i) if (dumpSteps[i] == stepNow) dump = true;
        if (dump)
        {
            std::printf("# STEP %d\n", int(stepNow));
            dumpVolS("end", rho);
            dumpVolUx("end", U);
            dumpVolS("end", thermo.T());
            dumpVolS("end", p);
            dumpVolS("end", e);
            dumpVolS("end", psi);
            dumpSurf("end", phi);
        }
    }

    Info<< "End\n" << endl;
    return 0;
}
