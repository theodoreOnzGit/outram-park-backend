// dumpLaplacianGeometry: ESI v2506 probe for gh:#1361 round 6. Research, education and V&V only.
// Written for this workspace; it only CALLS OpenFOAM.com v2506 (GPL-3.0) API, no upstream code is copied.
// Writes, into <case>/geometry/, one %.17g value per line: the geometric quantities the
// `Gauss linear uncorrected` laplacian of GeN-Foam's neutronics reads from the finite-volume mesh of
// region `neutroRegion`: V, magSf, linear weights, nonOrthDeltaCoeffs, deltaCoeffs (internal faces,
// then each boundary patch).
#include "argList.H"
#include "Time.H"
#include "fvMesh.H"
#include "surfaceInterpolation.H"
#include "surfaceFields.H"
#include "volFields.H"
#include "OSspecific.H"
#include <cstdio>

using namespace Foam;

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
    fvMesh mesh
    (
        IOobject(word("neutroRegion"), runTime.timeName(), runTime, IOobject::MUST_READ)
    );
    const fileName d = runTime.path()/"geometry";
    mkDir(d);
    dumpS(d, "V", mesh.V());
    dumpS(d, "magSf", mag(mesh.Sf()));
    dumpS(d, "weights", mesh.surfaceInterpolation::weights().primitiveField());
    dumpS(d, "nonOrthDeltaCoeffs", mesh.nonOrthDeltaCoeffs().primitiveField());
    dumpS(d, "deltaCoeffs", mesh.deltaCoeffs().primitiveField());
    forAll(mesh.boundary(), pi)
    {
        const word& nm = mesh.boundary()[pi].name();
        dumpS(d, "bnd_" + nm + "_nonOrthDeltaCoeffs", mesh.nonOrthDeltaCoeffs().boundaryField()[pi]);
        dumpS(d, "bnd_" + nm + "_deltaCoeffs", mesh.deltaCoeffs().boundaryField()[pi]);
        dumpS(d, "bnd_" + nm + "_patchDeltaCoeffs", mesh.boundary()[pi].deltaCoeffs());
        dumpS(d, "bnd_" + nm + "_magSf", mesh.boundary()[pi].magSf());
    }
    Info<< "wrote " << d << endl;
    return 0;
}
