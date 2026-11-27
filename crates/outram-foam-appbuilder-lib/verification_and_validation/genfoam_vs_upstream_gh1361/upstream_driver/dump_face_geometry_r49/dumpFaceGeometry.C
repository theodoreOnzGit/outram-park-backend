// dumpFaceGeometry: ESI v2506 probe for gh:#1897. Research, education and V&V only.
// Written for this workspace; it only CALLS OpenFOAM.com v2506 (GPL-3.0) API, no upstream code is copied.
// Reads the fvMesh of region neutroRegion and writes, %.17g, into <case>/geometry1897/:
// points, Sf, Cf (all faces), C, V (cells), weights (internal faces).
#include "argList.H"
#include "Time.H"
#include "fvMesh.H"
#include "surfaceInterpolation.H"
#include "surfaceFields.H"
#include "volFields.H"
#include "OSspecific.H"
#include "primitiveMeshTools.H"
#include <cstdio>
using namespace Foam;
static void dumpV(const fileName& d, const word& n, const vectorField& f)
{
    FILE* fp = fopen((d/n).c_str(), "w");
    forAll(f, i) fprintf(fp, "%.17g %.17g %.17g\n", f[i].x(), f[i].y(), f[i].z());
    fclose(fp);
}
static void dumpS(const fileName& d, const word& n, const scalarField& f)
{
    FILE* fp = fopen((d/n).c_str(), "w");
    forAll(f, i) fprintf(fp, "%.17g\n", f[i]);
    fclose(fp);
}
int main(int argc, char* argv[])
{
    #include "setRootCase.H"
    #include "createTime.H"
    fvMesh mesh(IOobject(word("neutroRegion"), runTime.timeName(), runTime, IOobject::MUST_READ));
    const fileName d = runTime.path()/"geometry1897";
    mkDir(d);
    dumpV(d, "points", mesh.points());
    dumpV(d, "Sf", mesh.faceAreas());
    dumpV(d, "Cf", mesh.faceCentres());
    dumpV(d, "C", mesh.cellCentres());
    dumpS(d, "V", mesh.cellVolumes());
    dumpS(d, "weights", mesh.surfaceInterpolation::weights().primitiveField());
    vectorField fc, fa;
    primitiveMeshTools::makeFaceCentresAndAreas(mesh.faces(), mesh.points(), fc, fa);
    label nd = 0;
    forAll(fa, i) { if (fa[i] != mesh.faceAreas()[i]) { nd++; } }
    Info<< "moving " << mesh.moving() << " recompute-differs " << nd << endl;
    return 0;
}
