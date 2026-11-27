/*---------------------------------------------------------------------------*\
  gh:#1784 (epic #1216, round 26) driver. NOT part of GeN-Foam: a small program
  written for the OUTRAM PARK differential comparison.

  What it measures: GeN-Foam 652b3da0cc55's `deformMesh()` (neutronics.C:363,
  thermalHydraulicsModel.C:380, thermoMechanics.C:353 are the same five lines):

      volPointInterpolation::New(mesh_).interpolate(<displacementField>)
      mesh_.movePoints(originalPoints_ + pointDisp.internalField())

  for every region named in multiRegionCouplingDict/meshDeformation, on
  OpenFOAM-ESI v2506's own volPointInterpolation and fvMesh::movePoints.

  * thermoMechanicalRegion goes through the REAL compiled
    solvers::legacyThermoMechanics::deformMesh() (libThermoMechanicsSolvers,
    built from the unmodified sources by build_and_run_legacy_tm_dump.sh).
  * every other region (neutroRegion, fluidRegion) runs the five upstream lines
    transcribed verbatim below (their solver classes need the neutronics /
    thermal-hydraulics libraries, which a mesh-motion probe does not need);
    the mechanics region is the check that the transcription equals the real
    method.

  What is NOT upstream: the displacement field. In a coupled run it is
  mapped from another region by meshToMesh; here its internal field is a
  closed-form function of the ORIGINAL cell centre (x, y, z) and the step
  number n, using only + - * / (exactly reproducible in the Rust port, no
  libm), set before every deformMesh() call:

      s  = (z + 0.2)/2.51      r2 = x*x + y*y
      dx = 1.0e-3*n*(x*s + 0.05*y*(1.0 - s))
      dy = 1.0e-3*n*(y*s - 0.05*x*s*s)
      dz = 2.0e-3*n*s*(1.0 + 0.3*r2)

  Boundary conditions are those of 0/<region>/disp (zeroGradient / fixedValue).
  Step 2 re-evaluates the field and moves from originalPoints_ again, so the
  mesh object's movePoints() re-weighting on the moved mesh is exercised.

  Usage:  meshDeformDump -case <case> <out.txt> <nSteps>
  Output: lines "F <step> <region>:<name> v1 v2 ..." at 17 significant digits
          (vectors flattened x y z), "P <region> n0 c.. n1 c.." for pointCells
          (count then cell ids per point), and "I <region> <flags>" for the
          primitiveMesh cache state seen BEFORE the first interpolation.

  Licence: GPL-3.0-or-later, as the OUTRAM PARK workspace; written fresh.
\*---------------------------------------------------------------------------*/

#include "argList.H"
#include "Time.H"
#include "fvCFD.H"
#include "volPointInterpolation.H"
#include "pointFields.H"
#include "legacyThermoMechanics.H"

#include <cstdio>
#include <cstdlib>

using namespace Foam;

static void vecField(FILE* f, label step, const word& region, const char* name, const vectorField& v)
{
    std::fprintf(f, "F %d %s:%s", int(step), region.c_str(), name);
    forAll(v, c) std::fprintf(f, " %.17g %.17g %.17g", v[c].x(), v[c].y(), v[c].z());
    std::fprintf(f, "\n");
}

static void scalField(FILE* f, label step, const word& region, const char* name, const scalarField& v)
{
    std::fprintf(f, "F %d %s:%s", int(step), region.c_str(), name);
    forAll(v, c) std::fprintf(f, " %.17g", v[c]);
    std::fprintf(f, "\n");
}

// the closed-form displacement of the header comment, on the ORIGINAL centres
static void setDisp(volVectorField& disp, const vectorField& C0, label step)
{
    const double n = double(step);
    forAll(C0, c)
    {
        const double x = C0[c].x(), y = C0[c].y(), z = C0[c].z();
        const double s = (z + 0.2)/2.51;
        const double r2 = x*x + y*y;
        disp[c] = vector
        (
            1.0e-3*n*(x*s + 0.05*y*(1.0 - s)),
            1.0e-3*n*(y*s - 0.05*x*s*s),
            2.0e-3*n*s*(1.0 + 0.3*r2)
        );
    }
    disp.correctBoundaryConditions();
}

static void dumpGeometry(FILE* f, label step, const word& region, const fvMesh& mesh)
{
    vecField(f, step, region, "points", mesh.points());
    vecField(f, step, region, "Cf", mesh.faceCentres());
    vecField(f, step, region, "Sf", mesh.faceAreas());
    vecField(f, step, region, "C", mesh.cellCentres());
    scalField(f, step, region, "V", mesh.cellVolumes());
}

static void dumpPointCells(FILE* f, const word& region, const fvMesh& mesh)
{
    const labelListList& pc = mesh.pointCells();
    std::fprintf(f, "P %s", region.c_str());
    forAll(pc, p)
    {
        std::fprintf(f, " %d", int(pc[p].size()));
        forAll(pc[p], i) std::fprintf(f, " %d", int(pc[p][i]));
    }
    std::fprintf(f, "\n");
}

// the real class, with the member the test needs (protected in upstream) exposed
class mechProbe : public solvers::legacyThermoMechanics
{
public:
    mechProbe(fvMesh& m) : solvers::legacyThermoMechanics(m) {}
    volVectorField& dispRef() { return disp_; }
    const pointField& original() const { return originalPoints_; }
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

    // multiRegionCouplingDict/meshDeformation, as deformMesh() reads it
    const IOdictionary couplingDict
    (
        IOobject("multiRegionCouplingDict", runTime.time().constant(), runTime.db(),
                 IOobject::MUST_READ, IOobject::NO_WRITE)
    );
    const dictionary deformDict(couplingDict.subDict("meshDeformation"));
    const wordList regions(deformDict.toc());

    FILE* f = std::fopen(out.c_str(), "w");

    PtrList<fvMesh> meshes(regions.size());
    PtrList<volVectorField> disps(regions.size());
    PtrList<pointField> originals(regions.size());
    PtrList<mechProbe> mech(regions.size());
    List<vectorField> C0(regions.size());

    forAll(regions, ri)
    {
        meshes.set(ri, new fvMesh(IOobject(regions[ri], runTime.name(), runTime, IOobject::MUST_READ)));
        fvMesh& mesh = meshes[ri];
        C0[ri] = mesh.C().field();
        if (regions[ri] == "thermoMechanicalRegion")
        {
            mech.set(ri, new mechProbe(mesh));
        }
        else
        {
            disps.set
            (
                ri,
                new volVectorField
                (
                    IOobject(deformDict.subDict(regions[ri]).get<word>("displacementField"),
                             runTime.timeName(), mesh, IOobject::MUST_READ, IOobject::NO_WRITE),
                    mesh
                )
            );
            originals.set(ri, new pointField(mesh.points()));   // neutronics.C:264 originalPoints_(mesh_.points())
        }
        std::fprintf(f, "I %s hasCellPoints=%d hasPointFaces=%d hasPointCells=%d hasCells=%d\n",
            regions[ri].c_str(), int(mesh.hasCellPoints()), int(mesh.hasPointFaces()),
            int(mesh.hasPointCells()), int(mesh.hasCells()));
    }

    for (label step = 1; step <= nSteps; ++step)
    {
        runTime++;
        Info<< "Time = " << runTime.timeName() << nl << endl;
        forAll(regions, ri)
        {
            fvMesh& mesh = meshes[ri];
            if (mech.set(ri))
            {
                mechProbe& tm = mech[ri];
                setDisp(tm.dispRef(), C0[ri], step);
                // the interpolated field the real deformMesh() is about to use (same weights: same geometry)
                const volPointInterpolation& vpi = volPointInterpolation::New(mesh);
                if (step == 1) dumpPointCells(f, regions[ri], mesh);
                tmp<pointVectorField> pd = vpi.interpolate(tm.dispRef());
                vecField(f, step, regions[ri], "pointDisp", pd->primitiveField());
                tm.deformMesh();    // the REAL solvers::thermoMechanics::deformMesh()
            }
            else
            {
                volVectorField& disp = disps[ri];
                setDisp(disp, C0[ri], step);
                // ---- upstream neutronics.C:386-403 / thermalHydraulicsModel.C:403-419, verbatim ----
                const volPointInterpolation& meshPointInterpolation = volPointInterpolation::New(mesh);
                if (step == 1) dumpPointCells(f, regions[ri], mesh);
                tmp<pointVectorField> meshPointsDisplacement = meshPointInterpolation.interpolate
                (
                    mesh.lookupObject<volVectorField>
                    (
                        deformDict.subDict(mesh.name()).get<word>("displacementField")
                    )
                );
                tmp<pointField> displacedPoints = originals[ri] + meshPointsDisplacement->internalField();
                mesh.movePoints(displacedPoints);
                // ---- end of the upstream lines ----
                vecField(f, step, regions[ri], "pointDisp", meshPointsDisplacement->primitiveField());
            }
            dumpGeometry(f, step, regions[ri], mesh);
        }
        std::fflush(f);
    }
    std::fclose(f);
    return 0;
}
