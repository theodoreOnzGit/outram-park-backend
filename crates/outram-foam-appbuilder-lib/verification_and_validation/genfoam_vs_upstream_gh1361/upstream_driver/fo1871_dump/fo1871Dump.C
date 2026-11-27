/*---------------------------------------------------------------------------*\
  gh:#1871 round 51 driver. NOT part of GeN-Foam: written for the OUTRAM PARK
  differential comparison. Constructs GeN-Foam 652b3da's OWN function objects
  (massFlow, TBulk, pressureDrop, patchScalarFieldValue, fieldDiffExtents,
  stopIfMaxFieldDiff; from the UNMODIFIED libFFSSrc2 on OpenFOAM-ESI v2506)
  through functionObject::New on a small box mesh, drives the fields they read
  from closed-form functions (only + - * /) and calls execute()/write() per
  step. The classes write their own postProcessing files (writePrecision 17);
  this driver adds the INPUTS and the run state at %.17g to <out.txt>.
  fieldIntegralToFMU is NOT compiled upstream (commDataLayer); only its
  statement `gSum(fieldZone*volZone)` is evaluated literally on a real zone.
  Lines: "M 0 <name> v..." mesh; "I <step> <name> v..." inputs;
         "O <step> <name> v"; "ORD <name> faces..." face order;
         "STOP <name> <step> <running>".
  Usage: fo1871Dump -case <case> <out.txt>
\*---------------------------------------------------------------------------*/

#include "argList.H"
#include "Time.H"
#include "fvCFD.H"
#include "rhoThermo.H"
#include "functionObject.H"
#include "faceSet.H"
#include "IOobjectList.H"
#include <cstdio>
#include <string>
#include <vector>

using namespace Foam;

static double fr(long a, long b, long i, long m)
{
    return double((a*1000003L + b*i) % m)/double(m);
}

static void prs(FILE* f, const char* tag, label step, const word& name, const scalarField& v)
{
    std::fprintf(f, "%s %d %s", tag, int(step), name.c_str());
    forAll(v, i) std::fprintf(f, " %.17g", v[i]);
    std::fprintf(f, "\n");
}

static void prv(FILE* f, const char* tag, label step, const word& name, const vectorField& v)
{
    std::fprintf(f, "%s %d %s", tag, int(step), name.c_str());
    forAll(v, i) std::fprintf(f, " %.17g %.17g %.17g", v[i].x(), v[i].y(), v[i].z());
    std::fprintf(f, "\n");
}

static void dumpVol(FILE* f, label step, const word& name, const volScalarField& v)
{
    prs(f, "I", step, name + ".cell", v.primitiveField());
    forAll(v.boundaryField(), p)
    {
        prs(f, "I", step, name + "." + v.mesh().boundary()[p].name(), v.boundaryField()[p]);
    }
}

static void dumpVolV(FILE* f, label step, const word& name, const volVectorField& v)
{
    prv(f, "I", step, name + ".cell", v.primitiveField());
    forAll(v.boundaryField(), p)
    {
        prv(f, "I", step, name + "." + v.mesh().boundary()[p].name(), v.boundaryField()[p]);
    }
}

int main(int argc, char* argv[])
{
    argList::noParallel();
    argList::addArgument("out");
    #include "setRootCase.H"
    #include "createTime.H"
    const fileName outName = args[1];
    FILE* f = std::fopen(outName.c_str(), "w");

    fvMesh mesh
    (
        IOobject(fvMesh::defaultRegion, runTime.timeName(), runTime, IOobject::MUST_READ)
    );
    autoPtr<rhoThermo> pThermo(rhoThermo::New(mesh));
    rhoThermo& thermo = pThermo();
    volScalarField& T = thermo.T();

    auto mkVol = [&](const word& n)
    {
        return new volScalarField
        (
            IOobject(n, runTime.timeName(), mesh, IOobject::NO_READ, IOobject::NO_WRITE),
            mesh, dimensionedScalar(n, dimless, 0.0), "calculated"
        );
    };
    volScalarField& prgh = *mkVol("p_rgh");
    volScalarField& A = *mkVol("A");
    volScalarField& B = *mkVol("B");
    volScalarField& SC1 = *mkVol("SC1");
    volScalarField& SC2 = *mkVol("SC2");
    volScalarField& SB1 = *mkVol("SB1");
    volScalarField& SB2 = *mkVol("SB2");
    volVectorField VA
    (
        IOobject("VA", runTime.timeName(), mesh, IOobject::NO_READ, IOobject::NO_WRITE),
        mesh, dimensionedVector("VA", dimless, vector::zero), "calculated"
    );
    volVectorField VB
    (
        IOobject("VB", runTime.timeName(), mesh, IOobject::NO_READ, IOobject::NO_WRITE),
        mesh, dimensionedVector("VB", dimless, vector::zero), "calculated"
    );
    surfaceScalarField arp
    (
        IOobject("alphaRhoPhi", runTime.timeName(), mesh, IOobject::NO_READ, IOobject::NO_WRITE),
        mesh, dimensionedScalar("alphaRhoPhi", dimless, 0.0), "calculated"
    );

    // mesh dump
    prs(f, "M", 0, "V", mesh.V());
    prv(f, "M", 0, "C", mesh.C());
    prv(f, "M", 0, "Cf", mesh.Cf().primitiveField());
    prv(f, "M", 0, "Sf", mesh.Sf().primitiveField());
    prs(f, "M", 0, "magSf", mesh.magSf().primitiveField());
    prs(f, "M", 0, "weights", mesh.weights().primitiveField());
    forAll(mesh.boundary(), p)
    {
        const word& pn = mesh.boundary()[p].name();
        prv(f, "M", 0, "Cf." + pn, mesh.Cf().boundaryField()[p]);
        prs(f, "M", 0, "magSf." + pn, mesh.magSf().boundaryField()[p]);
        prs(f, "M", 0, "deltaCoeffs." + pn, mesh.deltaCoeffs().boundaryField()[p]);
    }
    std::fprintf(f, "M 0 sigma %.17g\n", constant::physicoChemical::sigma.value());
    forAll(mesh.faceZones(), z)
    {
        std::fprintf(f, "ORD zone.%s", mesh.faceZones()[z].name().c_str());
        forAll(mesh.faceZones()[z], i) std::fprintf(f, " %d", int(mesh.faceZones()[z][i]));
        std::fprintf(f, "\n");
    }
    {
        IOobjectList objects
        (
            mesh,
            mesh.time().findInstance(polyMesh::meshSubDir/"sets", word::null, IOobject::READ_IF_PRESENT, mesh.facesInstance()),
            polyMesh::meshSubDir/"sets"
        );
        IOobjectList faceSets(objects.lookupClass(faceSet::typeName));
        const char* names[2] = {"fsA", "fsB"};
        for (int k = 0; k < 2; ++k)
        {
            Foam::faceSet set(*faceSets[names[k]]);
            std::fprintf(f, "ORD set.%s", names[k]);
            forAllIter(faceSet, set, iter) std::fprintf(f, " %d", int(iter.key()));
            std::fprintf(f, "\n");
        }
    }

    // function objects
    std::vector<std::pair<std::string, autoPtr<functionObject>>> fos;
    auto add = [&](const std::string& name, const std::string& body)
    {
        const std::string s = "type " + body + " writePrecision 17; log true; writeToFile true;";
        IStringStream is(s);
        dictionary d(is);
        fos.emplace_back(name, functionObject::New(name, runTime, d));
    };
    const char* patches[4] = {"inlet", "outlet", "walls", "caps"};
    for (int i = 0; i < 4; ++i)
    {
        const std::string p = patches[i];
        add("mf_" + p, "massFlow; regionType patch; regionName " + p + "; alphaRhoPhiName alphaRhoPhi;");
        add("mf180_" + p, "massFlow; regionType patch; regionName " + p + "; alphaRhoPhiName alphaRhoPhi; scaleFactor 180;");
        add("tb_" + p, "TBulk; regionType patch; regionName " + p + "; thermoName thermophysicalProperties; alphaRhoPhiName alphaRhoPhi;");
    }
    add("mf_zA", "massFlow; regionType faceZone; regionName zA; alphaRhoPhiName alphaRhoPhi; scaleFactor 180;");
    add("mf_zB", "massFlow; regionType faceZone; regionName zB; alphaRhoPhiName alphaRhoPhi;");
    add("mf_sA", "massFlow; regionType faceSet; regionName fsA; alphaRhoPhiName alphaRhoPhi;");
    add("mf_sB", "massFlow; regionType faceSet; regionName fsB; alphaRhoPhiName alphaRhoPhi; scaleFactor 180;");
    add("tb_zA", "TBulk; regionType faceZone; regionName zA; thermoName thermophysicalProperties; alphaRhoPhiName alphaRhoPhi;");
    add("tb_zB", "TBulk; regionType faceZone; regionName zB; thermoName thermophysicalProperties; alphaRhoPhiName alphaRhoPhi;");
    add("tb_sA", "TBulk; regionType faceSet; regionName fsA; thermoName thermophysicalProperties; alphaRhoPhiName alphaRhoPhi;");
    add("tb_sB", "TBulk; regionType faceSet; regionName fsB; thermoName thermophysicalProperties; alphaRhoPhiName alphaRhoPhi;");
    add("pd_pp", "pressureDrop; region1Type patch; region1 inlet; region2Type patch; region2 outlet;");
    add("pd_ss", "pressureDrop; region1Type faceSet; region1 fsA; region2Type faceSet; region2 fsB;");
    add("pd_ps", "pressureDrop; region1Type patch; region1 caps; region2Type faceSet; region2 fsB;");
    add("psv_inlet", "patchScalarFieldValue; field p_rgh; patch inlet;");
    add("psv_outlet", "patchScalarFieldValue; field p_rgh; patch outlet;");
    add("psv_caps", "patchScalarFieldValue; field B; patch caps;");
    add("fde", "fieldDiffExtents; fields (A VA); maskFields (B VB); referencePosition (0.1 0.2 0.3);");
    add("stopCells", "stopIfMaxFieldDiff; field1 SC1; field2 SC2;");
    add("stopBnd", "stopIfMaxFieldDiff; field1 SB1; field2 SB2;");

    const int nSteps = 12;
    for (int n = 1; n <= nSteps; ++n)
    {
        runTime++;
        forAll(mesh.C(), c)
        {
            T[c] = 300.0 + 100.0*fr(7, 3, c + 31*n, 97);
            prgh[c] = 1.0e5 + 1.0e3*fr(3, 7, c + 17*n, 101);
            const double bb = 10.0*fr(5, 11, c + 13*n, 89);
            B[c] = bb;
            const double d = (n == 4) ? -1.0 - fr(2, 3, c, 7) : ((c % 5 == 0) ? 0.0 : fr(9, 4, c + 3*n, 83) - 0.4);
            A[c] = bb + d;
            const double x = fr(1, 5, c + n, 79), y = fr(2, 7, c + 2*n, 73), z = fr(3, 2, c + 5*n, 71);
            VB[c] = vector(x, y, z);
            const double s = (n == 4) ? 0.5 : ((c % 5 == 0) ? 1.0 : 0.5 + fr(4, 9, c + 7*n, 67));
            VA[c] = vector(x*s, y*s, z*s);
            const double q = fr(6, 5, c + 11*n, 61);
            SC1[c] = (n >= 6) ? q - 0.3 : -0.1 - q;
            SC2[c] = 0.0;
            SB1[c] = -0.1 - q;
            SB2[c] = 0.0;
        }
        forAll(mesh.boundary(), p)
        {
            const fvPatch& pt = mesh.boundary()[p];
            const label np = pt.size();
            scalarField Tb(np), pb(np), Bb(np), Ab(np), s1(np), s2(np, 0.0), b1(np), b2(np, 0.0), ab(np);
            vectorField VBb(np), VAb(np);
            forAll(pt, i)
            {
                const long g = pt.start() + i;
                Tb[i] = 300.0 + 100.0*fr(11, 5, g + 29*n, 89);
                pb[i] = 1.0e5 + 1.0e3*fr(13, 2, g + 19*n, 97);
                const double bb = 10.0*fr(5, 11, g + 13*n, 89);
                Bb[i] = bb;
                const double d = (n == 4) ? -1.0 - fr(2, 3, g, 7) : ((g % 5 == 0) ? 0.0 : fr(9, 4, g + 3*n, 83) - 0.4);
                Ab[i] = bb + d;
                const double x = fr(1, 5, g + n, 79), y = fr(2, 7, g + 2*n, 73), z = fr(3, 2, g + 5*n, 71);
                VBb[i] = vector(x, y, z);
                const double s = (n == 4) ? 0.5 : ((g % 5 == 0) ? 1.0 : 0.5 + fr(4, 9, g + 7*n, 67));
                VAb[i] = vector(x*s, y*s, z*s);
                const double q = fr(6, 5, g + 11*n, 61);
                s1[i] = -0.5 - q;
                b1[i] = (n >= 9) ? q - 0.3 : -0.5 - q;
                ab[i] = (pt.name() == "walls") ? 0.0 : 2.0*fr(5, 3, g + 23*n, 103) - 0.8;
            }
            T.boundaryFieldRef()[p] == Tb;
            prgh.boundaryFieldRef()[p] == pb;
            B.boundaryFieldRef()[p] == Bb;
            A.boundaryFieldRef()[p] == Ab;
            VB.boundaryFieldRef()[p] == VBb;
            VA.boundaryFieldRef()[p] == VAb;
            SC1.boundaryFieldRef()[p] == s1;
            SC2.boundaryFieldRef()[p] == s2;
            SB1.boundaryFieldRef()[p] == b1;
            SB2.boundaryFieldRef()[p] == b2;
            arp.boundaryFieldRef()[p] == ab;
        }
        forAll(arp.primitiveFieldRef(), fi)
        {
            arp.primitiveFieldRef()[fi] = 2.0*fr(5, 3, fi + 23*n, 103) - 0.8;
        }
        tmp<volScalarField> tcp = thermo.Cp();
        dumpVol(f, n, "T", T);
        dumpVol(f, n, "Cp", tcp());
        dumpVol(f, n, "p_rgh", prgh);
        dumpVol(f, n, "A", A);
        dumpVol(f, n, "B", B);
        dumpVolV(f, n, "VA", VA);
        dumpVolV(f, n, "VB", VB);
        dumpVol(f, n, "SC1", SC1);
        dumpVol(f, n, "SB1", SB1);
        prs(f, "I", n, "arp.int", arp.primitiveField());
        forAll(arp.boundaryField(), p) prs(f, "I", n, "arp." + mesh.boundary()[p].name(), arp.boundaryField()[p]);

        for (auto& pr : fos)
        {
            pr.second->execute();
            pr.second->write();
            if (pr.first.substr(0, 4) == "stop")
            {
                std::fprintf(f, "STOP %s %d %d\n", pr.first.c_str(), n, int(runTime.value() < runTime.endTime().value() - 0.5*runTime.deltaTValue()));
                runTime.setEndTime(100);
                runTime.stopAt(Time::stopAtControls::saEndTime);
            }
        }
        // fieldIntegralToFMU: upstream's own statement, evaluated literally on the real cell zone
        {
            label id = mesh.cellZones().findZoneID("czA");
            const cellZone& z = mesh.cellZones()[id];
            scalarField fieldZone(A, z);
            scalarField volZone(mesh.V(), z);
            const scalar result = gSum(fieldZone*volZone);
            std::fprintf(f, "O %d fmu_integral %.17g\n", n, result);
            if (n == 1)
            {
                std::fprintf(f, "ORD cellzone.czA");
                forAll(z, i) std::fprintf(f, " %d", int(z[i]));
                std::fprintf(f, "\n");
            }
        }
    }
    std::fclose(f);
    return 0;
}
