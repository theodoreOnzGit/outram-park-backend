// trackParticles: ESI v2506 probe for the gh:#1382 particle-tracking differential.
// Research, education and V&V only. Written for this workspace; it only CALLS the
// OpenFOAM.com v2506 (GPL-3.0) class Foam::particle (liblagrangian), no upstream code
// is copied.
//
// Usage (in a case with constant/polyMesh and system/controlDict):
//   trackParticles -script <file>
// The script has one command per line (blank lines and '#' lines ignored):
//   new <x> <y> <z> <celli>                 particle(mesh, position, celli); locate()
//   reset <i>                               particle::reset()
//   track <i> <dx> <dy> <dz> <frac>         particle::track(displacement, fraction)
//   trackToFace <i> <dx> <dy> <dz> <frac>   particle::trackToFace(...)
// After every command one line is printed to stdout:
//   <cmd> <i> <ret> <a> <b> <c> <d> <celli> <tetFacei> <tetPti> <facei>
//       <stepFraction> <behind> <nBehind> <px> <py> <pz>
// with every double at %.17g ("-" in <ret> for commands without a return value).
//
// behind_ and nBehind_ are private in particle.H and have no accessor; the probe reads
// them through `#define private public` around the include (read only, same layout).
#include <cstdio>
#include <fstream>
#include <sstream>
#include <vector>
#include "argList.H"
#include "Time.H"
#include "polyMesh.H"
#include "meshTools.H"
#include "treeDataCell.H"
#include "indexedOctree.H"
#include "cubicEqn.H"
#include "transform.H"
#define private public
#include "particle.H"
#undef private

using namespace Foam;

static void emit
(
    const char* cmd,
    int idx,
    bool hasRet,
    double ret,
    const particle& p
)
{
    const barycentric& y = p.coordinates();
    const vector pos = p.position();
    if (hasRet) printf("%s %d %.17g", cmd, idx, ret);
    else printf("%s %d -", cmd, idx);
    printf
    (
        " %.17g %.17g %.17g %.17g %d %d %d %d %.17g %.17g %d %.17g %.17g %.17g\n",
        y.a(), y.b(), y.c(), y.d(),
        int(p.cell()), int(p.tetFace()), int(p.tetPt()), int(p.face()),
        p.stepFraction(), p.behind_, int(p.nBehind_),
        pos.x(), pos.y(), pos.z()
    );
}

int main(int argc, char *argv[])
{
    argList::addOption("script", "file", "particle command script");
    #include "setRootCase.H"
    #include "createTime.H"

    polyMesh mesh
    (
        IOobject
        (
            polyMesh::defaultRegion,
            runTime.timeName(),
            runTime,
            IOobject::MUST_READ
        )
    );

    const fileName scriptName(args.get<fileName>("script"));
    std::ifstream in(scriptName);
    std::vector<autoPtr<particle>> parts;

    std::string line;
    while (std::getline(in, line))
    {
        if (line.empty() || line[0] == '#') continue;
        std::istringstream ss(line);
        std::string cmd;
        ss >> cmd;
        if (cmd == "new")
        {
            double x, y, z; int celli;
            ss >> x >> y >> z >> celli;
            parts.push_back
            (
                autoPtr<particle>(new particle(mesh, vector(x, y, z), celli))
            );
            emit("new", int(parts.size()) - 1, false, 0, *parts.back());
        }
        else if (cmd == "reset")
        {
            int i; ss >> i;
            parts[i]->reset();
            emit("reset", i, false, 0, *parts[i]);
        }
        else if (cmd == "track" || cmd == "trackToFace")
        {
            int i; double dx, dy, dz, fr;
            ss >> i >> dx >> dy >> dz >> fr;
            const vector d(dx, dy, dz);
            const scalar r =
                cmd == "track"
              ? parts[i]->track(d, fr)
              : parts[i]->trackToFace(d, fr);
            emit(cmd.c_str(), i, true, r, *parts[i]);
        }
        else
        {
            FatalErrorInFunction << "unknown command " << cmd << exit(FatalError);
        }
    }
    return 0;
}
