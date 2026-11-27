/*---------------------------------------------------------------------------*\
  gh:#1852 / #1854 (epic #1216, round 30) driver. NOT part of GeN-Foam: a small
  program written for the OUTRAM PARK differential comparison.

  What it measures: the time-step sequence of GeN-Foam 652b3da0cc55
  (GeN-Foam.C:160-200, multiPhysicsSolver.C:174-186) on the 3D_SmallESFR meshes,
  with OpenFOAM-ESI v2506's own meshToMesh, fvMesh::movePoints, V0/V00, meshPhi
  and EulerDdtScheme::fvmDdt:

      runTime++;
      mapAllFields   : for each object of the shipped multiRegionCouplingDict,
                       meshToMesh(A, B, imCellVolumeWeight, pmAABB, false)
                       .mapTgtToSrc(B field, plusEqOp, A field); A field.correctBoundaryConditions()
                       (meshHandlerTemplates.C:85-93; the objects are built ONCE, before the loop,
                       on the undeformed meshes, as meshHandler.C:156-205)
      deformMesh()   : neutroRegion only (the shipped meshDeformation), the five upstream lines
                       of neutronics.C:386-403, verbatim, from the MAPPED `disp`
      (then the consumer: fvm::ddt(rho, flux) on the moving neutro mesh)

  meshHandler itself lives in a library that is not built here, so its map<>()
  is transcribed (the three lines above); the mechanics/fluid/neutro solvers are
  not run: their fields are closed-form (+ - * / only), set before every map.

  Calls: step 1 (call 1), step 2 (call 2), step 3 (call 3), and step 3 AGAIN
  with a changed meshDisp (call 4: a second Picard iteration in the same time
  index). deltaT = 0.05.

  Output lines (17 significant digits, vectors flattened):
    "F <call> <region>:<name> v ..."      fields (cells), name.bnd = all boundary values in patch order
    "S <call> moving=<0|1> timeIndex=<n>"
  Usage: coupledDeformDump -case <case> <out.txt>
\*---------------------------------------------------------------------------*/

#include "argList.H"
#include "Time.H"
#include "fvCFD.H"
#include "volPointInterpolation.H"
#include "pointFields.H"
#include "meshToMesh.H"
#include "fvm.H"

#include <cstdio>

using namespace Foam;

static void vecF(FILE* f, int call, const word& tag, const vectorField& v)
{
    fprintf(f, "F %d %s", call, tag.c_str());
    forAll(v, c) fprintf(f, " %.17g %.17g %.17g", v[c].x(), v[c].y(), v[c].z());
    fprintf(f, "\n");
}
static void scaF(FILE* f, int call, const word& tag, const scalarField& v)
{
    fprintf(f, "F %d %s", call, tag.c_str());
    forAll(v, c) fprintf(f, " %.17g", v[c]);
    fprintf(f, "\n");
}

template<class Type>
static Field<Type> flatBoundary(const GeometricField<Type, fvPatchField, volMesh>& f)
{
    label n = 0;
    forAll(f.boundaryField(), p) n += f.boundaryField()[p].size();
    Field<Type> out(n);
    label k = 0;
    forAll(f.boundaryField(), p) forAll(f.boundaryField()[p], i) out[k++] = f.boundaryField()[p][i];
    return out;
}

// closed-form fields: + - * / only
static double cf(const point& c, double n, double a)
{
    const double x = c.x(), y = c.y(), z = c.z();
    return a + 100.0*x - 40.0*y + 25.0*z + 10.0*x*y*z + 7.0*n*(x - 0.5*y) + a*n/3.0;
}
static vector dispOf(const point& c, double n)
{
    const double x = c.x(), y = c.y(), z = c.z();
    const double s = (z + 0.2)/2.51;
    const double r2 = x*x + y*y;
    return vector
    (
        1.0e-3*n*(x*s + 0.05*y*(1.0 - s)),
        1.0e-3*n*(y*s - 0.05*x*s*s),
        2.0e-3*n*s*(1.0 + 0.3*r2)
    );
}

template<class Type>
static tmp<GeometricField<Type, fvPatchField, volMesh>> makeField
(
    const word& name, const fvMesh& mesh, const Type& v0
)
{
    wordList types(mesh.boundary().size(), "zeroGradient");
    types[0] = "fixedValue";   // patch 0 of every field is fixedValue: a non-zeroGradient patch
    return tmp<GeometricField<Type, fvPatchField, volMesh>>
    (
        new GeometricField<Type, fvPatchField, volMesh>
        (
            IOobject(name, mesh.time().timeName(), mesh, IOobject::NO_READ, IOobject::NO_WRITE),
            mesh, dimensioned<Type>(name, dimless, v0), types
        )
    );
}

struct Obj { label recv; label prov; const char* src; const char* tgt; bool vec; };

int main(int argc, char* argv[])
{
    argList::noParallel();
    argList::addArgument("out");
    #include "setRootCase.H"
    #include "createTime.H"
    runTime.setDeltaT(0.05, false);
    FILE* f = fopen(args[1].c_str(), "w");

    const wordList names({"fluidRegion", "neutroRegion", "thermoMechanicalRegion"});
    PtrList<fvMesh> meshes(3);
    forAll(names, i) meshes.set(i, new fvMesh(IOobject(names[i], runTime.name(), runTime, IOobject::MUST_READ)));
    const label FL = 0, NE = 1, TM = 2;
    List<vectorField> C0(3);
    forAll(meshes, i) C0[i] = meshes[i].C().field();

    // the shipped dictionary, in its order (receiver region, provider region)
    const Obj objs[] = {
        {FL, NE, "powerDensity", "powerDensityNeutronics", false},
        {FL, NE, "secondaryPowerDensity", "powerDensityNeutronicsToLiquid", false},
        {NE, FL, "T", "TCool", false},
        {NE, FL, "thermo:rho", "rhoCool", false},
        {NE, FL, "T.fuelAvForNeutronics", "TFuel", false},
        {NE, FL, "T.cladAvForNeutronics", "TClad", false},
        {NE, FL, "T.passiveStructure", "TStructMech", false},
        {NE, TM, "meshDisp", "disp", true},
        {TM, FL, "T.passiveStructure", "TStructFromTH", false},
        {TM, FL, "T.fuelAvForNeutronics", "TFuel", false},
        {TM, NE, "powerDensity", "Q", false}
    };
    const int nObj = sizeof(objs)/sizeof(objs[0]);

    // meshToMesh objects, built once on the undeformed meshes, exactly as meshHandler.C:192-201
    const label pairs[5][2] = {{FL,NE},{NE,FL},{NE,TM},{TM,FL},{TM,NE}};
    PtrList<meshToMesh> m2m(5);
    forAll(m2m, k)
    {
        m2m.set(k, new meshToMesh(meshes[pairs[k][0]], meshes[pairs[k][1]],
            meshToMesh::interpolationMethod::imCellVolumeWeight, meshToMesh::procMapMethod::pmAABB, false));
    }
    auto pairIndex = [&](label r, label p) { for (int k = 0; k < 5; ++k) if (pairs[k][0]==r && pairs[k][1]==p) return k; return -1; };

    // fields: providers get closed-form values; receivers a different pre-existing closed form
    PtrList<volScalarField> sf; PtrList<volVectorField> vf;
    HashTable<label> sIdx, vIdx;
    auto key = [&](label r, const word& n) { return word(names[r] + ":" + n); };
    auto sField = [&](label r, const word& n, double a) -> volScalarField&
    {
        const word k = key(r, n);
        if (!sIdx.found(k)) { sIdx.insert(k, sf.size()); sf.resize(sf.size()+1); sf.set(sf.size()-1, makeField<scalar>(k, meshes[r], a).ptr()); }
        return sf[sIdx[k]];
    };
    auto vField = [&](label r, const word& n) -> volVectorField&
    {
        const word k = key(r, n);
        if (!vIdx.found(k)) { vIdx.insert(k, vf.size()); vf.resize(vf.size()+1); vf.set(vf.size()-1, makeField<vector>(k, meshes[r], vector::zero).ptr()); }
        return vf[vIdx[k]];
    };
    for (int o = 0; o < nObj; ++o)
    {
        if (objs[o].vec) { vField(objs[o].recv, objs[o].tgt); vField(objs[o].prov, objs[o].src); }
        else { sField(objs[o].recv, objs[o].tgt, 5.0 + o); sField(objs[o].prov, objs[o].src, 300.0 + 10.0*o); }
    }

    // consumer: fvm::ddt(rho, flux) on neutroRegion
    fvMesh& neutro = meshes[NE];
    const volPointInterpolation& vpi = volPointInterpolation::New(neutro);
    const pointField original(neutro.points());   // neutronics.C:264 originalPoints_(mesh_.points())
    volScalarField rho(makeField<scalar>("rho", neutro, 0.0)());
    volScalarField flux(makeField<scalar>("flux", neutro, 0.0)());
    forAll(rho, c) rho[c] = 1.0 + 0.1*neutro.C()[c].x();
    rho.correctBoundaryConditions();
    (void)rho.oldTime(); (void)flux.oldTime();

    auto mapAll = [&](double n, bool onlyDisp)
    {
        // providers
        for (int o = 0; o < nObj; ++o)
        {
            const Obj& ob = objs[o];
            if (onlyDisp && !ob.vec) continue;
            const point* dummy = nullptr; (void)dummy;
            if (ob.vec)
            {
                volVectorField& p = vField(ob.prov, ob.src);
                forAll(p, c) p[c] = dispOf(C0[ob.prov][c], n);
                p.correctBoundaryConditions();
            }
            else
            {
                volScalarField& p = sField(ob.prov, ob.src, 0);
                forAll(p, c) p[c] = cf(C0[ob.prov][c], n, 300.0 + 10.0*o);
                p.correctBoundaryConditions();
            }
        }
        // mapAllFields
        for (int o = 0; o < nObj; ++o)
        {
            const Obj& ob = objs[o];
            if (onlyDisp && !ob.vec) continue;
            const meshToMesh& m = m2m[pairIndex(ob.recv, ob.prov)];
            if (ob.vec)
            {
                volVectorField& tgt = vField(ob.recv, ob.tgt);
                m.mapTgtToSrc(vField(ob.prov, ob.src), plusEqOp<vector>(), tgt);   // meshHandlerTemplates.C:85
                tgt.correctBoundaryConditions();                                   // :93
            }
            else
            {
                volScalarField& tgt = sField(ob.recv, ob.tgt, 0);
                m.mapTgtToSrc(sField(ob.prov, ob.src, 0), plusEqOp<scalar>(), tgt);
                tgt.correctBoundaryConditions();
            }
        }
    };

    auto dumpMapped = [&](int call)
    {
        for (int o = 0; o < nObj; ++o)
        {
            const Obj& ob = objs[o];
            const word tag = key(ob.recv, ob.tgt);
            if (ob.vec)
            {
                volVectorField& t = vField(ob.recv, ob.tgt);
                vecF(f, call, tag, t.primitiveField()); vecF(f, call, tag + ".bnd", flatBoundary(t));
            }
            else
            {
                volScalarField& t = sField(ob.recv, ob.tgt, 0);
                scaF(f, call, tag, t.primitiveField()); scaF(f, call, tag + ".bnd", flatBoundary(t));
            }
        }
    };

    auto deformNeutro = [&](int call)
    {
        // ---- upstream neutronics.C:386-403, verbatim (displacementField = disp) ----
        tmp<pointVectorField> meshPointsDisplacement = vpi.interpolate(neutro.lookupObject<volVectorField>("neutroRegion:disp"));
        tmp<pointField> displacedPoints = original + meshPointsDisplacement->internalField();
        neutro.movePoints(displacedPoints);
        // ---- end ----
        fprintf(f, "S %d moving=%d timeIndex=%d\n", call, int(neutro.moving()), int(runTime.timeIndex()));
        vecF(f, call, "points", neutro.points());
        vecF(f, call, "oldPoints", neutro.oldPoints());
        scaF(f, call, "V", neutro.cellVolumes());
        scaF(f, call, "V0", neutro.V0().field());
        scaF(f, call, "V00", neutro.V00().field());   // lazily built from V0 on the first request, then maintained
        const surfaceScalarField& mp = neutro.phi();
        scaF(f, call, "meshPhi", mp.primitiveField());
        {
            label n = 0; forAll(mp.boundaryField(), p) n += mp.boundaryField()[p].size();
            scalarField b(n); label k = 0;
            forAll(mp.boundaryField(), p) forAll(mp.boundaryField()[p], i) b[k++] = mp.boundaryField()[p][i];
            scaF(f, call, "meshPhi.bnd", b);
        }
        // gh:#1856 item 5: meshPhi.oldTime() (the previous time index's last meshPhi; a copy of the current one when first asked)
        scaF(f, call, "meshPhiOld", mp.oldTime().primitiveField());
        // the consumer
        // primitiveFieldRef() calls storeOldTimes(): the first write of a time index saves the previous value as oldTime()
        {
            scalarField& fr = flux.primitiveFieldRef();
            forAll(fr, c) fr[c] = cf(C0[NE][c], double(call), 1.0);
        }
        fvScalarMatrix ddt(fvm::ddt(rho, flux));
        scaF(f, call, "ddtDiag", ddt.diag());
        scaF(f, call, "ddtSource", ddt.source());
        scaF(f, call, "fluxOld", flux.oldTime().primitiveField());
    };

    // call 1..3
    for (int step = 1; step <= 3; ++step)
    {
        runTime++;
        mapAll(double(step), false);
        dumpMapped(step);
        if (step == 1)
        {
            // reverse direction (heatExchanger uses mapSrcToTgt): receiver field `disp` onto the provider's list
            vectorField back(meshes[TM].nCells());
            forAll(back, c) back[c] = vector(0.5 + c*1e-7, 2.0, -1.0);
            m2m[pairIndex(NE, TM)].mapSrcToTgt(vField(NE, "disp").primitiveField(), plusEqOp<vector>(), back);
            vecF(f, step, "reverseVector", back);
            scalarField backS(meshes[FL].nCells());
            forAll(backS, c) backS[c] = 1.0 + c*1e-7;
            m2m[pairIndex(NE, FL)].mapSrcToTgt(sField(NE, "TCool", 0).primitiveField(), plusEqOp<scalar>(), backS);
            scaF(f, step, "reverseScalar", backS);
        }
        deformNeutro(step);
        if (step == 3)
        {
            // a second Picard iteration in the same time index: new meshDisp, mapped, deformMesh() again
            mapAll(3.5, true);
            vecF(f, 4, key(NE, "disp"), vField(NE, "disp").primitiveField());
            vecF(f, 4, key(NE, "disp") + ".bnd", flatBoundary(vField(NE, "disp")));
            deformNeutro(4);
        }
        fflush(f);
    }
    fclose(f);
    return 0;
}
