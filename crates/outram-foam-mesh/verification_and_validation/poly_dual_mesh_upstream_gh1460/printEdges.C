// Scratch diagnostic (not part of the port): print mesh.edges() and
// pointEdges numbering of OpenFOAM.com v2506 for a case.
#include "argList.H"
#include "Time.H"
#include "fvMesh.H"
using namespace Foam;
int main(int argc, char *argv[])
{
    #include "setRootCase.H"
    #include "createTime.H"
    #include "createNamedMesh.H"
    const edgeList& e = mesh.edges();
    forAll(e, i)
    {
        Info<< "E " << i << ' ' << e[i][0] << ' ' << e[i][1] << nl;
    }
    return 0;
}
