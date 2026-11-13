// dumpQuality: ESI v2506 probe for the gh:#1364 checkMesh differential.
// Research, education and V&V only. Written for this workspace; it only CALLS
// OpenFOAM.com v2506 (GPL-3.0) primitiveMeshTools, no upstream code is copied.
// Writes, into <case>/quality/, one text file per field, one %.17g value per line
// (vectors: three values per line), from the mesh in <case>/constant/polyMesh.
#include "argList.H"
#include "Time.H"
#include "polyMesh.H"
#include "primitiveMeshTools.H"
#include "bitSet.H"
#include "unitConversion.H"
#include "OSspecific.H"
#include <cstdio>

using namespace Foam;

static void dumpS(const fileName& d, const word& n, const scalarField& f)
{
    FILE* fp = fopen((d/n).c_str(), "w");
    forAll(f, i) fprintf(fp, "%.17g\n", f[i]);
    fclose(fp);
}
static void dumpV(const fileName& d, const word& n, const vectorField& f)
{
    FILE* fp = fopen((d/n).c_str(), "w");
    forAll(f, i) fprintf(fp, "%.17g %.17g %.17g\n", f[i].x(), f[i].y(), f[i].z());
    fclose(fp);
}

int main(int argc, char* argv[])
{
    #include "setRootCase.H"
    #include "createTime.H"
    polyMesh mesh
    (
        IOobject(polyMesh::defaultRegion, runTime.timeName(), runTime, IOobject::MUST_READ)
    );
    const fileName d = runTime.path()/"quality";
    mkDir(d);
    const Vector<label>& meshD = mesh.geometricD();
    {
        FILE* fp = fopen((d/"meshD").c_str(), "w");
        fprintf(fp, "%d %d %d\n", int(meshD.x()), int(meshD.y()), int(meshD.z()));
        fclose(fp);
    }
    const vectorField& fC = mesh.faceCentres();
    const vectorField& fA = mesh.faceAreas();
    const vectorField& cC = mesh.cellCentres();
    const scalarField& cV = mesh.cellVolumes();
    dumpV(d, "faceCentres", fC);
    dumpV(d, "faceAreas", fA);
    dumpV(d, "cellCentres", cC);
    dumpS(d, "cellVolumes", cV);
    dumpS(d, "ortho", primitiveMeshTools::faceOrthogonality(mesh, fA, cC)());
    dumpS(d, "skew", primitiveMeshTools::faceSkewness(mesh, mesh.points(), fC, fA, cC)());
    scalarField own, nei;
    primitiveMeshTools::facePyramidVolume(mesh, mesh.points(), cC, own, nei);
    dumpS(d, "ownPyr", own);
    dumpS(d, "neiPyr", nei);
    scalarField openness, aratio;
    primitiveMeshTools::cellClosedness(mesh, meshD, fA, cV, openness, aratio);
    dumpS(d, "openness", openness);
    dumpS(d, "aspect", aratio);
    dumpS(d, "flatness", primitiveMeshTools::faceFlatness(mesh, mesh.points(), fC, fA)());
    dumpS(d, "concavity", primitiveMeshTools::faceConcavity(Foam::sin(degToRad(10.0)), mesh, mesh.points(), fA)());
    bitSet internal(mesh.nFaces());
    for (label i = 0; i < mesh.nInternalFaces(); ++i) internal.set(i);
    dumpS(d, "determinant", primitiveMeshTools::cellDeterminant(mesh, meshD, fA, internal)());
    Info<< "dumpQuality: " << mesh.nCells() << " cells, " << mesh.nFaces() << " faces" << endl;
    return 0;
}
