// dumpMeshMotion: ESI v2506 probe for the gh:#1380 dynamic-mesh differential.
// Research, education and V&V only. Written for this workspace; it only CALLS
// OpenFOAM.com v2506 (GPL-3.0) classes (dynamicFvMesh::update, fvMesh::phi), no
// upstream code is copied. The time loop is the one of `moveDynamicMesh`.
// After every mesh.update() it writes, into <case>/probe/<timeName>/, one text
// file per field, one %.17g value per line (vectors: three values per line):
//   points, meshPhi (all faces: internal then each patch in order),
//   faceCentres, faceAreas, cellCentres, cellVolumes.
#include "argList.H"
#include "Time.H"
#include "dynamicFvMesh.H"
#include "pimpleControl.H"
#include "surfaceFields.H"
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

int main(int argc, char *argv[])
{
    #include "setRootCase.H"
    #include "createTime.H"
    #include "createNamedDynamicFvMesh.H"

    pimpleControl pimple(mesh);

    while (runTime.loop())
    {
        Info<< "Time = " << runTime.timeName() << endl;

        while (pimple.loop())
        {
            if (pimple.firstIter())
            {
                mesh.update();
            }
        }

        const fileName d = runTime.path()/"probe"/runTime.timeName();
        mkDir(d);

        const surfaceScalarField& mphi = mesh.phi();
        scalarField all(mesh.nFaces(), 0.0);
        forAll(mphi.primitiveField(), i) all[i] = mphi.primitiveField()[i];
        forAll(mphi.boundaryField(), pi)
        {
            const fvsPatchScalarField& pf = mphi.boundaryField()[pi];
            const label start = mesh.boundaryMesh()[pi].start();
            forAll(pf, i) all[start + i] = pf[i];
        }
        dumpS(d, "meshPhi", all);
        dumpV(d, "points", mesh.points());
        dumpV(d, "faceCentres", mesh.faceCentres());
        dumpV(d, "faceAreas", mesh.faceAreas());
        dumpV(d, "cellCentres", mesh.cellCentres());
        dumpS(d, "cellVolumes", mesh.cellVolumes());
    }

    Info<< "dumpMeshMotion: " << mesh.nCells() << " cells, " << mesh.nFaces()
        << " faces" << endl;
    return 0;
}
