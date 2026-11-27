/*---------------------------------------------------------------------------*\
  gh:#1230 round 35 driver. NOT part of GeN-Foam: a small program written for
  the OUTRAM PARK differential comparison. It constructs GeN-Foam 652b3da0cc55's
  OWN `structure` (and through it the real `powerModel` of the type the case
  selects: lumpedNuclearStructure, heatedPin, nuclearFuelPin,
  interpolatedNuclearFuelPin; libFFSSrc2 built from the unmodified sources on
  OpenFOAM-ESI v2506) on a small mesh with cell zones, exactly as
  `structure(dict, mesh, powerDensityNeutronics)` is called by the solvers.

  What is NOT upstream: in a coupled run `powerDensityNeutronics`, `H` and `HT`
  come from the neutronics and the fluid; here they are written from closed-form
  functions of the cell index c, the step n and the outer iteration k that use
  only + - * / (exactly reproducible in the Rust port, no libm):
     q  = q0 (1 + 0.05 c)(1 + 0.1 n)
     H  = H0 (1 + 0.04 c)(1 + 0.02 n)
     Tf = Tf0 + 3 c + 5 n + 0.5 k
     HT = H Tf
  q0, H0, Tf0, nSteps, nOuter come from constant/couplingProperties.
  `structure::correct(HT, H)` is called nOuter times per step (the outer
  iterations of a PIMPLE loop). A derived class exposes the protected members.

  Usage:  powerModelDump -case <case> <structureProperties file> <out.txt>
  Output: <out.txt>, lines
     "S <step> <outer> <name> v1 v2 ..."   volScalarField over all cells
     "R <step> <outer> <name> <cell> v1 v2 ..." per-cell radial/nodal profile
     "D <step> <outer> <type> <name> v"    powerModel dictionary scalar
  at 17 significant digits. The solver log is on stdout.

  Licence: GPL-3.0-or-later, as the OUTRAM PARK workspace; written fresh.
\*---------------------------------------------------------------------------*/

#include "argList.H"
#include "Time.H"
#include "IFstream.H"
#include "fvCFD.H"
#include "structure.H"
#include "IOFieldFields.H"

#include <cstdio>
#include <cstdlib>
#include <string>
#include <vector>

using namespace Foam;

class dumpStructure : public structure
{
public:
    using structure::structure;

    const auto& pms() const { return powerModels_; }
    const labelList& cellsOf() const { return cells_; }
};

static void scal(FILE* f, label step, label outer, const word& name, const volScalarField& v)
{
    std::fprintf(f, "S %d %d %s", int(step), int(outer), name.c_str());
    forAll(v, c) std::fprintf(f, " %.17g", v[c]);
    std::fprintf(f, "\n");
}

static void prof
(
    FILE* f, label step, label outer, const fvMesh& mesh, const labelList& cells,
    const word& name
)
{
    if (!mesh.foundObject<IOFieldField<Field, scalar>>(name)) return;
    const IOFieldField<Field, scalar>& ff = mesh.lookupObject<IOFieldField<Field, scalar>>(name);
    forAll(cells, i)
    {
        const label c = cells[i];
        std::fprintf(f, "R %d %d %s %d", int(step), int(outer), name.c_str(), int(c));
        forAll(ff[c], j) std::fprintf(f, " %.17g", ff[c][j]);
        std::fprintf(f, "\n");
    }
}

int main(int argc, char* argv[])
{
    argList::noParallel();
    argList::addArgument("structureDict");
    argList::addArgument("out");
    #include "setRootCase.H"
    #include "createTime.H"
    const fileName dictFile(args[1]);
    const fileName out(args[2]);

    IFstream dictStream(dictFile);
    const dictionary structDict(dictStream());

    IFstream cplStream(runTime.path()/"constant"/"couplingProperties");
    const dictionary cpl(cplStream());
    const double q0 = cpl.get<scalar>("q0");
    const double H0 = cpl.get<scalar>("H0");
    const double Tf0 = cpl.get<scalar>("Tf0");
    const label nSteps = cpl.get<label>("nSteps");
    const label nOuter = cpl.get<label>("nOuter");
    // optional: a time-dependent power switch is NOT modelled; powerOff is not called

    fvMesh mesh
    (
        IOobject(fvMesh::defaultRegion, runTime.timeName(), runTime, IOobject::MUST_READ)
    );

    volScalarField powerDensityNeutronics
    (
        IOobject("powerDensityNeutronics", runTime.timeName(), mesh, IOobject::NO_READ, IOobject::NO_WRITE),
        mesh, dimensionedScalar("", dimPower/dimVol, 0.0), zeroGradientFvPatchScalarField::typeName
    );
    volScalarField H
    (
        IOobject("H", runTime.timeName(), mesh, IOobject::NO_READ, IOobject::NO_WRITE),
        mesh, dimensionedScalar("", dimPower/dimArea/dimTemperature, 0.0), zeroGradientFvPatchScalarField::typeName
    );
    volScalarField HT
    (
        IOobject("HT", runTime.timeName(), mesh, IOobject::NO_READ, IOobject::NO_WRITE),
        mesh, dimensionedScalar("", dimPower/dimArea, 0.0), zeroGradientFvPatchScalarField::typeName
    );

    dumpStructure st(structDict, mesh, powerDensityNeutronics);

    FILE* f = std::fopen(out.c_str(), "w");
    std::fprintf(f, "H deltaT %.17g nCells %d nSteps %d nOuter %d\n", runTime.deltaTValue(), int(mesh.nCells()), int(nSteps), int(nOuter));

    // initial state (before any correct): iA of each model and Tact
    scal(f, 0, 0, "Tact0", st.Tact());
    scal(f, 0, 0, "iAact", st.iAact());

    for (label step = 1; step <= nSteps; ++step)
    {
        runTime++;
        Info<< "Time = " << runTime.timeName() << nl << endl;
        for (label k = 0; k < nOuter; ++k)
        {
            const double n = double(step);
            const double kk = double(k);
            forAll(mesh.C(), c)
            {
                const double cc = double(c);
                powerDensityNeutronics[c] = q0*(1.0 + 0.05*cc)*(1.0 + 0.1*n);
                const double h = H0*(1.0 + 0.04*cc)*(1.0 + 0.02*n);
                const double tf = Tf0 + 3.0*cc + 5.0*n + 0.5*kk;
                H[c] = h;
                HT[c] = h*tf;
            }
            powerDensityNeutronics.correctBoundaryConditions();
            H.correctBoundaryConditions();
            HT.correctBoundaryConditions();

            // algebraic models: interpolatedHeatFlux is an external field (meshHandler mapping)
            if (mesh.foundObject<volScalarField>("interpolatedHeatFlux"))
            {
                volScalarField& ihf = const_cast<volScalarField&>(mesh.lookupObject<volScalarField>("interpolatedHeatFlux"));
                forAll(ihf, c) ihf[c] = 1.0e4*(1.0 + 0.1*double(c))*(1.0 + 0.05*n);
                ihf.correctBoundaryConditions();
            }

            st.correct(HT, H);

            scal(f, step, k, "Tact", st.Tact());
            scal(f, step, k, "TFuelAv", st.TFuelAv());
            scal(f, step, k, "TCladAv", st.TCladAv());
            scal(f, step, k, "Twall", st.Twall());
            scal(f, step, k, "heatFlux", mesh.lookupObject<volScalarField>("heatFlux.structure"));

            forAllConstIters(st.pms(), iter)
            {
                const word type = iter.key();
                const powerModel& pm = *(iter.val());
                const labelList& cells = st.cellsOf();
                (void)cells;
                // cells of this model: those with iA > 0
                labelList mine;
                forAll(pm.iA(), c) if (pm.cellField()[c] > 0.5) mine.append(c);
                scal(f, step, k, "iA." + type, pm.iA());
                const word suffix = "." + type;
                const char* vols[] =
                {
                    "Tmax", "Tmatrix", "Tsurface", "laplacianTmatrix", "kappaMatrixField",
                    "powerDensity", "Ti", "To", "Tav", "Tfi", "Tfo", "Tci", "Tco", "Tfav", "Tcav",
                    "interpolatedHeatFlux", "interpolatedPowerDensity", "T"
                };
                for (const char* nm : vols)
                {
                    const word full = word(nm) + suffix;
                    if (mesh.foundObject<volScalarField>(full))
                    {
                        scal(f, step, k, full, mesh.lookupObject<volScalarField>(full));
                    }
                }
                // the interpolated model's two fields are not group-named upstream
                const char* plain[] = {"interpolatedHeatFlux", "interpolatedPowerDensity"};
                for (const char* nm : plain)
                {
                    if (type == "interpolatedNuclearFuelPin" && mesh.foundObject<volScalarField>(nm))
                    {
                        scal(f, step, k, nm, mesh.lookupObject<volScalarField>(nm));
                    }
                }
                prof(f, step, k, mesh, mine, "T" + suffix);
                prof(f, step, k, mesh, mine, "Trad" + suffix);
                const char* dicts[] = {"Tavav", "Tmax", "Tmin", "Tfavav", "Tcavav", "Tfmax", "Tfmin", "Tcmax", "Tcmin"};
                for (const char* nm : dicts)
                {
                    if (pm.found(nm))
                    {
                        std::fprintf(f, "D %d %d %s %s %.17g\n", int(step), int(k), type.c_str(), nm, double(pm.get<scalar>(nm)));
                    }
                }
            }
        }
        std::fflush(f);
    }
    std::fclose(f);
    return 0;
}
