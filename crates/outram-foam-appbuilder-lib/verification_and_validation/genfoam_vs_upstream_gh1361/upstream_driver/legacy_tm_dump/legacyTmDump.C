/*---------------------------------------------------------------------------*\
  gh:#1781 (epic #1216, round 23) driver. NOT part of GeN-Foam: a small program
  written for the OUTRAM PARK differential comparison. It constructs GeN-Foam
  652b3da0cc55's OWN `solvers::legacyThermoMechanics` (libThermoMechanicsSolvers
  built from the unmodified sources on OpenFOAM-ESI v2506; see
  scripts/build_legacy_tm_dump.sh), exactly as `meshHandler`/`regionSolvers` do
  (`fvMesh(IOobject(regionName, runTime.name(), runTime, MUST_READ))`), and
  calls `correctPhysics()` once per step, as `GeN-Foam.C` does.

  What is NOT upstream: in a coupled run `TStructFromTH`, `TFuel` and
  `powerDensityNeutronics` come from the other regions through `meshToMesh`;
  here a derived class (the members are `protected`) writes them from the
  closed-form functions of the cell centre and the step number below, which use
  only + - * / (exactly reproducible in the Rust port, no libm). The Rust test
  evaluates the same expressions in the same order.

  Usage:  legacyTmDump -case <case> <out.txt> <nSteps>
  Output: <out.txt>, lines "F <step> <name> v1 v2 ..." at 17 significant
          digits (a vector field flattened x y z per cell); the solver log on
          stdout.

  Licence: GPL-3.0-or-later, as the OUTRAM PARK workspace; written fresh.
\*---------------------------------------------------------------------------*/

#include "argList.H"
#include "Time.H"
#include "fvCFD.H"
#include "legacyThermoMechanics.H"

#include <cstdio>
#include <cstdlib>

using namespace Foam;

class probe : public solvers::legacyThermoMechanics
{
public:
    probe(fvMesh& m) : solvers::legacyThermoMechanics(m) {}

    void setCoupling(label step)
    {
        const double n = double(step);
        const labelList& inner = mesh_.cellZones()[mesh_.cellZones().findZoneID("innerCore")];
        const labelList& outer = mesh_.cellZones()[mesh_.cellZones().findZoneID("outerCore")];
        const labelList& foll = mesh_.cellZones()[mesh_.cellZones().findZoneID("follower")];
        const labelList& rod = mesh_.cellZones()[mesh_.cellZones().findZoneID("controlRod")];
        List<char> isCore(mesh_.nCells(), 0), isPor(mesh_.nCells(), 0);
        forAll(inner, i) { isCore[inner[i]] = 1; isPor[inner[i]] = 1; }
        forAll(outer, i) { isCore[outer[i]] = 1; isPor[outer[i]] = 1; }
        forAll(foll, i) { isPor[foll[i]] = 1; }
        forAll(rod, i) { isPor[rod[i]] = 1; }
        const vectorField& C = mesh_.C();
        forAll(C, c)
        {
            const double x = C[c].x(), y = C[c].y(), z = C[c].z();
            const double s = (z + 0.2)/2.51;
            const double r2 = x*x + y*y;
            const double shape = 4.0*s*(1.0 - s);
            TStructFromTH_[c] = isPor[c] ? 700.0 + 300.0*shape + 25.0*r2 + 10.0*n*s : 0.0;
            TFuel_[c] = isCore[c] ? 900.0 + 400.0*shape*(1.0 - 0.1*r2) + 30.0*n*(1.0 - 0.25*r2) : 0.0;
            powerDensityNeutronics_[c] = 1.0e5*(1.0 + 0.5*s + 0.1*n)*(1.0 + 0.2*r2);
        }
        TStructFromTH_.correctBoundaryConditions();
        TFuel_.correctBoundaryConditions();
        powerDensityNeutronics_.correctBoundaryConditions();
    }

    // gh:#1845 (round 27): `dispOnly` writes only `disp` (the other fields are bit-checked at the last step), to keep the reference small
    void dump(FILE* f, label step, bool dispOnly = false) const
    {
        if (dispOnly)
        {
            vec(f, step, "disp", disp_);
            return;
        }
        scal(f, step, "TStruct", TStruct_);
        vec(f, step, "disp", disp_);
        scal(f, step, "fuelDisp", fuelDisp_);
        scal(f, step, "CRDisp", CRDisp_);
        vec(f, step, "meshDisp", meshDisp_);
        // gh:#1235 (round 24): the boundary value and the gradient of every GFTractionDisplacement patch
        forAll(disp_.boundaryField(), pi)
        {
            const fvPatchVectorField& pf = disp_.boundaryField()[pi];
            if (pf.type() == "GFTractionDisplacement")
            {
                const fixedGradientFvPatchVectorField& fg = refCast<const fixedGradientFvPatchVectorField>(pf);
                vecField(f, step, (word("dispB_") + pf.patch().name()).c_str(), pf);
                vecField(f, step, (word("gradB_") + pf.patch().name()).c_str(), fg.gradient());
            }
        }
    }

private:
    static void vecField(FILE* f, label step, const char* name, const vectorField& v)
    {
        std::fprintf(f, "F %d %s", int(step), name);
        forAll(v, c) std::fprintf(f, " %.17g %.17g %.17g", v[c].x(), v[c].y(), v[c].z());
        std::fprintf(f, "\n");
    }
    static void scal(FILE* f, label step, const char* name, const volScalarField& v)
    {
        std::fprintf(f, "F %d %s", int(step), name);
        forAll(v, c) std::fprintf(f, " %.17g", v[c]);
        std::fprintf(f, "\n");
    }
    static void vec(FILE* f, label step, const char* name, const volVectorField& v)
    {
        std::fprintf(f, "F %d %s", int(step), name);
        forAll(v, c) std::fprintf(f, " %.17g %.17g %.17g", v[c].x(), v[c].y(), v[c].z());
        std::fprintf(f, "\n");
    }
};

int main(int argc, char* argv[])
{
    argList::noParallel();
    argList::addArgument("out");
    argList::addArgument("nSteps");
    #include "setRootCase.H"
    #include "createTime.H"
    const fileName out(args[1]);
    const label nSteps = atoi(args[2].c_str());

    fvMesh mesh(IOobject("thermoMechanicalRegion", runTime.name(), runTime, IOobject::MUST_READ));
    probe tm(mesh);
    FILE* f = std::fopen(out.c_str(), "w");
    // gh:#1845 (round 27): LEGACY_TM_DT_RATIO=g multiplies deltaT by g before every step from the 2nd on
    // (`Time::setDeltaT` before `runTime++`, as an adjustable-step run does), so deltaT0 != deltaT in the d2dt2 scheme;
    // LEGACY_TM_DISP_ONLY=1 dumps `disp` at every step and every field at the last step only.
    const char* ratioEnv = std::getenv("LEGACY_TM_DT_RATIO");
    const double ratio = ratioEnv ? std::atof(ratioEnv) : 1.0;
    const bool dispOnly = std::getenv("LEGACY_TM_DISP_ONLY") != nullptr;
    for (label step = 1; step <= nSteps; ++step)
    {
        if (step >= 2 && ratio != 1.0)
        {
            runTime.setDeltaT(runTime.deltaTValue()*ratio);
        }
        runTime++;
        std::fprintf(f, "F %d deltaT %.17g\nF %d deltaT0 %.17g\n", int(step), runTime.deltaTValue(), int(step), runTime.deltaT0Value());
        Info<< "Time = " << runTime.timeName() << nl << endl;
        tm.setCoupling(step);
        tm.correctPhysics();
        Info<< "residual " << tm.getResidual() << nl;
        tm.dump(f, step, dispOnly && step < nSteps);
        std::fflush(f);
    }
    std::fclose(f);
    return 0;
}
