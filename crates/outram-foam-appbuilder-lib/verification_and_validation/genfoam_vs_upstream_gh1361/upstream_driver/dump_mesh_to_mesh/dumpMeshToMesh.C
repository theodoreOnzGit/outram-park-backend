// Round 9 (issue 1774): the addressing and weights of GeN-Foam's `meshHandler` mapping between the two
// IDENTICAL 2D_MSFR meshes: meshToMesh(src, tgt, imCellVolumeWeight, pmAABB, normalise = false), exactly the
// call at meshHandler.C:192. Dumps, per direction, the number of overlaps per cell and, for each, the address
// and the weight (`%.17g`) into <case>/geometry/m2m_*.
#include "argList.H"
#include "Time.H"
#include "fvMesh.H"
#include "meshToMesh.H"
#include "OSspecific.H"
#include <cstdio>

using namespace Foam;

static void dumpList(const fileName& dir, const word& name, const labelListList& a, const scalarListList& w)
{
    FILE* fp = fopen((dir/name).c_str(), "w");
    forAll(a, i)
    {
        fprintf(fp, "%d", int(a[i].size()));
        forAll(a[i], j) fprintf(fp, " %d %.17g", int(a[i][j]), w[i][j]);
        fprintf(fp, "\n");
    }
    fclose(fp);
}

int main(int argc, char* argv[])
{
    #include "setRootCase.H"
    #include "createTime.H"
    fvMesh fluid(IOobject(word("fluidRegion"), runTime.timeName(), runTime, IOobject::MUST_READ));
    fvMesh neutro(IOobject(word("neutroRegion"), runTime.timeName(), runTime, IOobject::MUST_READ));
    const fileName dir = runTime.path()/"geometry";
    mkDir(dir);
    meshToMesh fn(fluid, neutro, meshToMesh::interpolationMethod::imCellVolumeWeight, meshToMesh::procMapMethod::pmAABB, false);
    meshToMesh nf(neutro, fluid, meshToMesh::interpolationMethod::imCellVolumeWeight, meshToMesh::procMapMethod::pmAABB, false);
    dumpList(dir, "m2m_fluid_neutro_srcToTgt", fn.srcToTgtCellAddr(), fn.srcToTgtCellWght());
    dumpList(dir, "m2m_fluid_neutro_tgtToSrc", fn.tgtToSrcCellAddr(), fn.tgtToSrcCellWght());
    dumpList(dir, "m2m_neutro_fluid_srcToTgt", nf.srcToTgtCellAddr(), nf.srcToTgtCellWght());
    dumpList(dir, "m2m_neutro_fluid_tgtToSrc", nf.tgtToSrcCellAddr(), nf.tgtToSrcCellWght());
    Info<< "overlap volume " << fn.V() << " " << nf.V() << nl << "wrote " << dir << endl;
    return 0;
}
