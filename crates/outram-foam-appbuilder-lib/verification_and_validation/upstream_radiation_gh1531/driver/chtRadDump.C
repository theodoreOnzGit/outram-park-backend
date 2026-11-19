/*---------------------------------------------------------------------------*\
  gh:#1531: the gh:#1363 driver (upstream chtMultiRegionFoam, OpenFOAM-v2506,
  ESI, one frozen-flow fluid + one solid) run with the P1 radiation model ON in
  the fluid. Identical to ../../upstream_openfoam_gh1363/driver/chtDump.C except
  for the added dump block at the end of the time loop (G in the cells, qr and G
  on the boundary patches); `rad.Sh(thermo, he)` and `rad.correct()` were already
  in the transcribed EEqn.H.  Original header follows.

  gh:#1363 row 4: upstream chtMultiRegionFoam (OpenFOAM-v2506, ESI) on a
  two-region case (one frozen-flow fluid, one solid), as a dump driver.

  This is `applications/solvers/heatTransfer/chtMultiRegionFoam/
  chtMultiRegionFoam.C` with createMeshes.H, createFluidFields.H,
  createSolidFields.H, setRegionFluidFields.H, setRegionSolidFields.H,
  solveFluid.H (the frozenFlow branch), fluid/EEqn.H and solid/solveSolid.H
  transcribed in order, on the UNMODIFIED upstream libraries, including the
  real thermo (heRhoThermo with const transport, hConst, rhoConst; heSolidThermo
  with constIso), the real laminar compressible::turbulenceModel, the real
  radiationModel (none), fv::options, and the real coupled boundary condition
  compressible::turbulentTemperatureRadCoupledMixed (libchtBCs, built from
  src/thermoTools, see scripts/build_cht_bcs.sh). Substitutions, all of them:

    1. rhoReactionThermo / CombustionModel (libcombustionModels is not built)
       are replaced by rhoThermo::New; the case has one pure fluid, so there is
       no composition and no YEqn.
    2. MRF, pressureControl, UEqn, pEqn, rhoEqn and the PISO loop are omitted:
       the fluid has `frozenFlow yes`, so solveFluid.H runs only EEqn.H, which
       is transcribed in full (h-form). Its `U` is zero.
    3. The coupled-energy assembly (`coupledEnergyField`), the extra
       "energyCoupling" loops and the Courant/diffusion-number time-step control
       are omitted (the case uses neither; deltaT is fixed).
    4. `runTime.write()` is replaced by hexadecimal dumps.

  Output lines (printed to stdout, one value per line):
    S <step> <tag> <name> <i> <hex>   per-cell or per-face value, IEEE-754 bits
    C <step> <name> <hex>             a scalar
\*---------------------------------------------------------------------------*/
#include "fvCFD.H"
#include "rhoThermo.H"
#include "turbulentFluidThermoModel.H"
#include "solidThermo.H"
#include "radiationModel.H"
#include "fvOptions.H"
#include "regionProperties.H"
#include "gravityMeshObject.H"
#include "mixedFvPatchFields.H"
#include "emptyPolyPatch.H"
#include "IOdictionary.H"
#include <cstring>
#include <cstdio>
#include <cstdint>

using namespace Foam;

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

static void hexC(const char* name, double v)
{
    std::uint64_t b;
    std::memcpy(&b, &v, sizeof(b));
    std::printf("C %d %s %016llx\n", int(stepNow), name, (unsigned long long)b);
}

// Heat into the region through patch pi [W]: sum(kappa_face magSf snGrad(T)),
// the quantity the coupled BC prints as "heat transfer rate".
static double heatIn
(
    const volScalarField& T,
    const volScalarField& kappa,
    label pi
)
{
    const fvPatchScalarField& Tp = T.boundaryField()[pi];
    const scalarField kp(kappa.boundaryField()[pi]);
    const scalarField sn(Tp.snGrad());
    const scalarField magSf(T.mesh().boundary()[pi].magSf());
    double q = 0;
    forAll(sn, i) q += kp[i]*magSf[i]*sn[i];
    return q;
}

// The interface face temperature the mixed condition implies from the
// coefficients used in this step's matrix and the NEW cell temperature,
// Tf = vf refValue + (1 - vf) (Tc + refGrad/deltaCoeffs). (The stored
// T.boundaryField() lags the BC coefficients by one step, because the energy
// solve evaluates the h condition from the previous face value; both are
// dumped, as "Tb" and "Tf".)
static scalarField faceT(const volScalarField& T, label pi)
{
    const mixedFvPatchScalarField& m =
        refCast<const mixedFvPatchScalarField>(T.boundaryField()[pi]);
    const fvPatch& patch = T.mesh().boundary()[pi];
    const scalarField Tc(m.patchInternalField());
    return m.valueFraction()*m.refValue()
         + (1.0 - m.valueFraction())*(Tc + m.refGrad()/patch.deltaCoeffs());
}

int main(int argc, char *argv[])
{
    #include "setRootCase.H"
    #include "createTime.H"

    IOdictionary ctrl
    (
        IOobject
        (
            "controlDict", runTime.system(), runTime,
            IOobject::MUST_READ, IOobject::NO_WRITE
        )
    );
    const word ifaceName(ctrl.get<word>("interfacePatch"));

    regionProperties rp(runTime);
    const wordList fluidNames(rp["fluid"]);
    const wordList solidNames(rp["solid"]);

    // createFluidMeshes.H / createSolidMeshes.H (one region each)
    fvMesh mesh(IOobject(fluidNames[0], runTime.timeName(), runTime, IOobject::MUST_READ));
    fvMesh smesh(IOobject(solidNames[0], runTime.timeName(), runTime, IOobject::MUST_READ));

    // createFluidFields.H
    const uniformDimensionedVectorField& g = meshObjects::gravity::New(runTime);
    (void)g;
    autoPtr<rhoThermo> pThermo(rhoThermo::New(mesh));
    rhoThermo& thermo = pThermo();
    thermo.validate(args.executable(), "h", "e");

    volScalarField rho
    (
        IOobject("rho", runTime.timeName(), mesh, IOobject::NO_READ, IOobject::NO_WRITE),
        thermo.rho()
    );
    volVectorField U
    (
        IOobject("U", runTime.timeName(), mesh, IOobject::MUST_READ, IOobject::NO_WRITE),
        mesh
    );
    surfaceScalarField phi
    (
        IOobject("phi", runTime.timeName(), mesh, IOobject::READ_IF_PRESENT, IOobject::NO_WRITE),
        linearInterpolate(rho*U) & mesh.Sf()
    );
    autoPtr<compressible::turbulenceModel> pTurbulence
    (
        compressible::turbulenceModel::New(rho, U, phi, thermo)
    );
    compressible::turbulenceModel& turbulence = pTurbulence();
    autoPtr<radiation::radiationModel> pRad(radiation::radiationModel::New(thermo.T()));
    radiation::radiationModel& rad = pRad();
    volScalarField K("K", 0.5*magSqr(U));
    volScalarField dpdt
    (
        IOobject("dpdt", runTime.timeName(), mesh),
        mesh,
        dimensionedScalar(thermo.p().dimensions()/dimTime, Zero)
    );
    volScalarField Qdot
    (
        IOobject("Qdot", runTime.timeName(), mesh, IOobject::READ_IF_PRESENT, IOobject::NO_WRITE),
        mesh,
        dimensionedScalar(dimEnergy/dimVolume/dimTime, Zero)
    );
    fv::options fvOptions(mesh);
    turbulence.validate();

    // createSolidFields.H
    autoPtr<solidThermo> pSThermo(solidThermo::New(smesh));
    solidThermo& sthermo = pSThermo();
    autoPtr<radiation::radiationModel> pSRad(radiation::radiationModel::New(sthermo.T()));
    fv::options sOptions(smesh);
    volScalarField betav
    (
        IOobject("betavSolid", runTime.timeName(), smesh, IOobject::NO_READ, IOobject::NO_WRITE),
        smesh,
        dimensionedScalar("1", dimless, scalar(1))
    );

    const label fpi = mesh.boundaryMesh().findPatchID(ifaceName);
    const label spi = smesh.boundaryMesh().findPatchID(ifaceName);

    // the constants the port is given: upstream's own numbers
    {
        tmp<volScalarField> tk = thermo.kappa();
        tmp<volScalarField> tcp = thermo.Cp();
        tmp<volScalarField> tsk = sthermo.kappa();
        tmp<volScalarField> tscp = sthermo.Cp();
        tmp<volScalarField> tsrho = sthermo.rho();
        hexC("fluid_kappa", tk()[0]);
        hexC("fluid_Cp", tcp()[0]);
        hexC("fluid_rho", rho[0]);
        hexC("fluid_alphaEff", turbulence.alphaEff()()[0]);
        hexC("solid_kappa", tsk()[0]);
        hexC("solid_Cp", tscp()[0]);
        hexC("solid_rho", tsrho()[0]);
        hexC("solid_alpha", sthermo.alpha()[0]);
    }

    const bool frozenFlow =
        mesh.solutionDict().subDict("PIMPLE").getOrDefault<bool>("frozenFlow", false);
    if (!frozenFlow)
    {
        FatalErrorInFunction << "this driver transcribes only the frozenFlow branch" << exit(FatalError);
    }
    fvSolution solutionDict(runTime);
    const int nOuterCorr = solutionDict.subDict("PIMPLE").getOrDefault<int>("nOuterCorrectors", 1);
    if (nOuterCorr != 1)
    {
        FatalErrorInFunction << "this driver transcribes nOuterCorrectors 1" << exit(FatalError);
    }

    while (runTime.run())
    {
        ++runTime;
        ++stepNow;
        Info<< "Time = " << runTime.timeName() << nl << endl;

        for (int oCorr = 0; oCorr < nOuterCorr; ++oCorr)
        {
            const bool finalIter = (oCorr == nOuterCorr - 1);

            // --- fluid region: solveFluid.H, frozenFlow branch -> EEqn.H
            {
                if (finalIter) mesh.data().setFinalIteration(true);

                volScalarField& he = thermo.he();
                volScalarField& p = thermo.p();
                (void)p;
                fvScalarMatrix EEqn
                (
                    fvm::ddt(rho, he) + fvm::div(phi, he)
                  + fvc::ddt(rho, K) + fvc::div(phi, K)
                  + (
                        he.name() == "e"
                      ? fvc::div
                        (
                            fvc::absolute(phi/fvc::interpolate(rho), U),
                            p,
                            "div(phiv,p)"
                        )
                      : -dpdt
                    )
                  - fvm::laplacian(turbulence.alphaEff(), he)
                 ==
                    rho*(U & g)
                  + rad.Sh(thermo, he)
                  + Qdot
                  + fvOptions(rho, he)
                );
                EEqn.relax();
                fvOptions.constrain(EEqn);
                EEqn.solve(he.select(finalIter));
                fvOptions.correct(he);
                thermo.correct();
                rad.correct();
                Info<< "Min/max T:" << min(thermo.T()).value() << ' '
                    << max(thermo.T()).value() << endl;

                if (finalIter) mesh.data().setFinalIteration(false);
            }

            // --- solid region: solveSolid.H
            {
                tmp<volScalarField> trho = sthermo.rho();
                const volScalarField& srho = trho();
                volScalarField& h = sthermo.he();
                fvScalarMatrix hEqn
                (
                    fvm::ddt(betav*srho, h)
                  - fvm::laplacian(betav*sthermo.alpha(), h, "laplacian(alpha,h)")
                 ==
                    sOptions(srho, h)
                );
                hEqn.relax();
                sOptions.constrain(hEqn);
                if (finalIter) smesh.data().setFinalIteration(true);
                hEqn.solve(h.select(finalIter));
                sOptions.correct(h);
                sthermo.correct();
                Info<< "Min/max T:" << min(sthermo.T()).value() << ' '
                    << max(sthermo.T()).value() << endl;
                if (finalIter) smesh.data().setFinalIteration(false);
            }
        }

        // dumps
        dumpScalar("f", "T", thermo.T().primitiveField());
        dumpScalar("f", "h", thermo.he().primitiveField());
        dumpScalar("s", "T", sthermo.T().primitiveField());
        dumpScalar("s", "h", sthermo.he().primitiveField());
        dumpScalar("f", "Tb", thermo.T().boundaryField()[fpi]);
        dumpScalar("s", "Tb", sthermo.T().boundaryField()[spi]);
        dumpScalar("f", "Tf", faceT(thermo.T(), fpi));
        dumpScalar("s", "Tf", faceT(sthermo.T(), spi));
        if (rad.radiation())
        {
            // gh:#1541: a viewFactor fluid has no G; the P1 output is unchanged when G exists
            const bool hasG = mesh.foundObject<volScalarField>("G");
            const volScalarField& qr = mesh.lookupObject<volScalarField>("qr");
            if (hasG) dumpScalar("f", "G", mesh.lookupObject<volScalarField>("G").primitiveField());
            forAll(mesh.boundaryMesh(), pi)
            {
                const polyPatch& pp = mesh.boundaryMesh()[pi];
                if (isA<emptyPolyPatch>(pp)) continue;
                const word nm(pp.name());
                dumpScalar("f", ("qr_" + nm).c_str(), qr.boundaryField()[pi]);
                if (hasG)
                {
                    dumpScalar("f", ("Gb_" + nm).c_str(), mesh.lookupObject<volScalarField>("G").boundaryField()[pi]);
                }
            }
        }
        {
            tmp<volScalarField> tk = thermo.kappa();
            tmp<volScalarField> tsk = sthermo.kappa();
            hexC("fluid_Qin", heatIn(thermo.T(), tk(), fpi));
            hexC("solid_Qin", heatIn(sthermo.T(), tsk(), spi));
        }
    }

    Info<< "End\n" << endl;
    return 0;
}
