/*---------------------------------------------------------------------------*\
  gh:#1363 row 6: ESI v2506's `solidificationMelting` fvOption
  (src/fvOptions/sources/derived/solidificationMeltingSource) on a 1-D
  conduction slab, as a dump driver.

  ESI v2506 has no solidification/melting SOLVER; the physics is the
  `solidificationMelting` fvOption, attached to a solver's energy equation. The
  only shipped use is the solid `ice` region of chtMultiRegionFoam's
  windshieldDefrost. This driver is therefore the SOLID energy equation of
  chtMultiRegionFoam (`solid/solveSolid.H`, transcribed as in chtDump.C) on ONE
  region, on the unmodified libraries: the real heSolidThermo (constIso, hConst,
  rhoConst, sensibleEnthalpy), the real fv::options reading the real
  solidificationMelting option. There is no flow, so the option's momentum
  contribution (Darcy drag, Boussinesq buoyancy) is NOT exercised.

  Substitutions: radiation and coordinate systems are omitted (isotropic, no
  radiation), `betav` is 1, and `runTime.write()` is replaced by hexadecimal
  dumps.

  Output lines as in chtDump.C, plus
    S <step> m alpha1 <i> <hex>   the option's liquid fraction after the step
\*---------------------------------------------------------------------------*/
#include "fvCFD.H"
#include "solidThermo.H"
#include "fvOptions.H"
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

int main(int argc, char *argv[])
{
    #include "setRootCase.H"
    #include "createTime.H"
    #include "createMesh.H"

    autoPtr<solidThermo> pThermo(solidThermo::New(mesh));
    solidThermo& thermo = pThermo();
    fv::options fvOptions(mesh);
    volScalarField betav
    (
        IOobject("betavSolid", runTime.timeName(), mesh, IOobject::NO_READ, IOobject::NO_WRITE),
        mesh,
        dimensionedScalar("1", dimless, scalar(1))
    );

    {
        tmp<volScalarField> tk = thermo.kappa();
        tmp<volScalarField> tcp = thermo.Cp();
        tmp<volScalarField> trho = thermo.rho();
        hexC("kappa", tk()[0]);
        hexC("Cp", tcp()[0]);
        hexC("rho", trho()[0]);
        hexC("alpha", thermo.alpha()[0]);
    }

    while (runTime.run())
    {
        ++runTime;
        ++stepNow;
        Info<< "Time = " << runTime.timeName() << nl << endl;

        // solid/solveSolid.H, non-coupled branch, nOuterCorrectors 1
        {
            tmp<volScalarField> trho = thermo.rho();
            const volScalarField& rho = trho();
            volScalarField& h = thermo.he();
            fvScalarMatrix hEqn
            (
                fvm::ddt(betav*rho, h)
              - fvm::laplacian(betav*thermo.alpha(), h, "laplacian(alpha,h)")
             ==
                fvOptions(rho, h)
            );
            hEqn.relax();
            fvOptions.constrain(hEqn);
            mesh.data().setFinalIteration(true);
            hEqn.solve(h.select(true));
            fvOptions.correct(h);
            thermo.correct();
            mesh.data().setFinalIteration(false);
            Info<< "Min/max T:" << min(thermo.T()).value() << ' '
                << max(thermo.T()).value() << endl;
        }

        dumpScalar("m", "T", thermo.T().primitiveField());
        dumpScalar("m", "h", thermo.he().primitiveField());
        const volScalarField& a1 = mesh.lookupObject<volScalarField>("melt:alpha1");
        dumpScalar("m", "alpha1", a1.primitiveField());
    }

    Info<< "End\n" << endl;
    return 0;
}
