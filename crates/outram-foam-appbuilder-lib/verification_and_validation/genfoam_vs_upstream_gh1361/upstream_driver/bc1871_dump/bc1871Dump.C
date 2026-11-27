/*---------------------------------------------------------------------------*\
  gh:#1871 round 51 driver. NOT part of GeN-Foam: written for the OUTRAM PARK
  differential comparison. Reads fields whose patches are GeN-Foam's OWN
  blackBodyRadiation, timeFieldTable and velocityRundown boundary conditions
  (UNMODIFIED libFFSSrc2, OpenFOAM-ESI v2506) on the 24-cell box of
  make_fo1871_case.sh, drives the cell values from closed-form functions
  (only + - * /), calls correctBoundaryConditions() for 12 steps (t = 1..12)
  and prints each patch value at %.17g.
  Lines: "B <step> <name> v..."; "M <name> v..." (sigma, deltaCoeffs, patchInternalField).
  Usage: bc1871Dump -case <case> <out.txt>
\*---------------------------------------------------------------------------*/

#include "argList.H"
#include "Time.H"
#include "fvCFD.H"
#include "constants.H"
#include <cstdio>

using namespace Foam;

static double fr(long a, long b, long i, long m)
{
    return double((a*1000003L + b*i) % m)/double(m);
}

static void prs(FILE* f, const char* tag, int step, const word& name, const scalarField& v)
{
    std::fprintf(f, "%s %d %s", tag, step, name.c_str());
    forAll(v, i) std::fprintf(f, " %.17g", v[i]);
    std::fprintf(f, "\n");
}

static void prv(FILE* f, const char* tag, int step, const word& name, const vectorField& v)
{
    std::fprintf(f, "%s %d %s", tag, step, name.c_str());
    forAll(v, i) std::fprintf(f, " %.17g %.17g %.17g", v[i].x(), v[i].y(), v[i].z());
    std::fprintf(f, "\n");
}

int main(int argc, char* argv[])
{
    argList::noParallel();
    argList::addArgument("out");
    #include "setRootCase.H"
    #include "createTime.H"
    FILE* f = std::fopen(args[1].c_str(), "w");

    fvMesh mesh
    (
        IOobject(fvMesh::defaultRegion, runTime.timeName(), runTime, IOobject::MUST_READ)
    );
    const label pOut = mesh.boundaryMesh().findPatchID("outlet");
    const label pIn = mesh.boundaryMesh().findPatchID("inlet");

    volScalarField Tbb
    (
        IOobject("Tbb", "0", mesh, IOobject::MUST_READ, IOobject::NO_WRITE), mesh
    );
    volScalarField Ttab
    (
        IOobject("Ttab", "0", mesh, IOobject::MUST_READ, IOobject::NO_WRITE), mesh
    );
    volVectorField Urun
    (
        IOobject("Urun", "0", mesh, IOobject::MUST_READ, IOobject::NO_WRITE), mesh
    );
    autoPtr<volVectorField> Urun2;

    std::fprintf(f, "M 0 sigma %.17g\n", constant::physicoChemical::sigma.value());
    prs(f, "M", 0, "deltaCoeffs.outlet", mesh.deltaCoeffs().boundaryField()[pOut]);
    prs(f, "M", 0, "Tbb.initial", Tbb.boundaryField()[pOut]);
    prs(f, "M", 0, "Ttab.initial", Ttab.boundaryField()[pIn]);
    prv(f, "M", 0, "Urun.initial", Urun.boundaryField()[pIn]);

    const int nSteps = 12;
    for (int n = 1; n <= nSteps; ++n)
    {
        runTime++;
        if (n == 6)
        {
            // constructed AFTER t0 = 3: velocityRundown's constructor divides U0 by the rundown factor
            Urun2.reset
            (
                new volVectorField
                (
                    IOobject("Urun2", "0", mesh, IOobject::MUST_READ, IOobject::NO_WRITE), mesh
                )
            );
            prv(f, "B", n, "Urun2.constructed", Urun2().boundaryField()[pIn]);
        }
        forAll(Tbb, c)
        {
            Tbb[c] = 300.0 + 400.0*fr(7, 3, c + 31*n, 97);
        }
        Tbb.correctBoundaryConditions();
        Ttab.correctBoundaryConditions();
        Urun.correctBoundaryConditions();
        if (Urun2.valid()) Urun2.ref().correctBoundaryConditions();

        prs(f, "B", n, "Ti.outlet", Tbb.boundaryField()[pOut].patchInternalField());
        prs(f, "B", n, "Tbb.outlet", Tbb.boundaryField()[pOut]);
        prs(f, "B", n, "Ttab.inlet", Ttab.boundaryField()[pIn]);
        prv(f, "B", n, "Urun.inlet", Urun.boundaryField()[pIn]);
        if (Urun2.valid()) prv(f, "B", n, "Urun2.inlet", Urun2().boundaryField()[pIn]);
    }
    std::fclose(f);
    return 0;
}
