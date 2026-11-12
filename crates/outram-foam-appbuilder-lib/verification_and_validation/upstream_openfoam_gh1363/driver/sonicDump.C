/*---------------------------------------------------------------------------*\
  gh:#1363 row 3b: upstream sonicFoam (OpenFOAM-v2506, ESI) on the laminar
  shockTube, as a dump driver.

  This is `applications/solvers/compressible/sonicFoam/sonicFoam.C` with its
  four .H files (createFields, UEqn, EEqn, pEqn) transcribed line for line,
  built on the UNMODIFIED upstream libraries, including the real
  psiThermo::New (hePsiThermo, const transport, hConst, perfectGas,
  sensibleInternalEnergy) and the real compressible::turbulenceModel::New
  (laminar Stokes). Substitutions, all of them:

    1. `fvOptions` and `MRF` are omitted (libfvOptions and its transitive
       closure are not built, see the V&V record). The shockTube has no
       fvOptions file and no MRFProperties, so both are inactive upstream too.
    2. `runTime.write()` is replaced by hexadecimal dumps of the fields at the
       steps listed in controlDict key `dumpSteps`, plus (when
       `dumpStages yes;`) the intermediate fields after each equation of the
       first `dumpStageSteps` steps, so a divergence can be located by
       its first differing stage.
    3. The continuity-error block of compressibleContinuityErrs.H is printed
       as IEEE-754 bit patterns as well as in the usual text.

  Output lines (stdout):
    # STEP <n>
    S <step> <tag> <field> <index> <16 hex digits>
    C <step> <name> <16 hex digits>
\*---------------------------------------------------------------------------*/

#include "fvCFD.H"
#include "psiThermo.H"
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
    autoPtr<psiThermo> pThermo(psiThermo::New(mesh));
    psiThermo& thermo = pThermo();

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

    mesh.setFluxRequired(p.name());

    Info<< "Creating turbulence model\n" << endl;
    autoPtr<compressible::turbulenceModel> turbulence
    (
        compressible::turbulenceModel::New(rho, U, phi, thermo)
    );

    #include "createK.H"

    // ---- createFieldRefs.H
    volScalarField& e = thermo.he();
    const volScalarField& psi = thermo.psi();

    #include "initContinuityErrs.H"

    turbulence->validate();

    Info<< "\nStarting time loop\n" << endl;

    while (runTime.loop())
    {
        stepNow = runTime.timeIndex();
        dumpStagesNow = dumpStages && stepNow <= dumpStageSteps;
        Info<< "Time = " << runTime.timeName() << nl << endl;

        #include "compressibleCourantNo.H"
        solveRhoEqn(rho, phi);          // rhoEqn.H without fvOptions
        if (dumpStagesNow) dumpVolS("rhoEqn", rho);

        while (pimple.loop())
        {
            // ---- UEqn.H
            fvVectorMatrix UEqn
            (
                fvm::ddt(rho, U) + fvm::div(phi, U)
              + turbulence->divDevRhoReff(U)
            );

            UEqn.relax();

            if (pimple.momentumPredictor())
            {
                solve(UEqn == -fvc::grad(p));
                K = 0.5*magSqr(U);
            }
            if (dumpStagesNow) dumpVolUx("UEqn", U);

            // ---- EEqn.H
            {
                fvScalarMatrix EEqn
                (
                    fvm::ddt(rho, e) + fvm::div(phi, e)
                  + fvc::ddt(rho, K) + fvc::div(phi, K)
                  + fvc::div
                    (
                        fvc::absolute(phi/fvc::interpolate(rho), U),
                        p,
                        "div(phiv,p)"
                    )
                  - fvm::laplacian(turbulence->alphaEff(), e)
                );

                EEqn.relax();
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

                volScalarField rAU(1.0/UEqn.A());
                surfaceScalarField rhorAUf("rhorAUf", fvc::interpolate(rho*rAU));
                volVectorField HbyA("HbyA", U);
                HbyA = rAU*UEqn.H();

                surfaceScalarField phid
                (
                    "phid",
                    fvc::interpolate(psi)
                   *(
                        fvc::flux(HbyA)
                      + rhorAUf*fvc::ddtCorr(rho, U, phi)/fvc::interpolate(rho)
                    )
                );

                if (dumpStagesNow)
                {
                    dumpVolS("pEqn:rAU", rAU);
                    dumpVolUx("pEqn:HbyA", HbyA);
                    dumpSurf("pEqn:phid", phid);
                    dumpSurf("pEqn:rhorAUf", rhorAUf);
                }

                while (pimple.correctNonOrthogonal())
                {
                    fvScalarMatrix pEqn
                    (
                        fvm::ddt(psi, p)
                      + fvm::div(phid, p)
                      - fvm::laplacian(rhorAUf, p)
                    );

                    pEqn.solve();

                    if (pimple.finalNonOrthogonalIter())
                    {
                        phi = pEqn.flux();
                    }
                }

                solveRhoEqn(rho, phi);   // rhoEqn.H without fvOptions
                #include "compressibleContinuityErrs.H"
                {
                    // bit patterns of the same three numbers
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

                U = HbyA - rAU*fvc::grad(p);
                U.correctBoundaryConditions();
                K = 0.5*magSqr(U);

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
