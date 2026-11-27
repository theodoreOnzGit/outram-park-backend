# gh:#1895 round 46: splits the hx zone of the 2D_MSFR mesh into hxA/hxB of equal volume (cells ordered by (z, y), best balanced cut) and writes the cellZones overlay. Carries the author's scratch paths (/home/user/upstream-build/gh1895/c = gunzipped cases/msfr_neutro/polyMesh + boundary). usage: python3 -I make_msfr_th_hx_two_zones.py 2
import re, sys, math
d='/home/user/upstream-build/gh1895/c/constant/polyMesh/'
def body(f):
    s=open(d+f).read()
    i=s.index('FoamFile'); j=s.index('}',i)
    return s[j+1:]
pts=[tuple(map(float,m)) for m in re.findall(r'\(([-\d.eE+]+) ([-\d.eE+]+) ([-\d.eE+]+)\)', body('points'))]
fb=body('faces')
faces=[list(map(int,m.split())) for m in re.findall(r'\d+\(([\d ]+)\)', fb)]
own=list(map(int,re.findall(r'\n(\d+)(?=\n)', body('owner').split('(',1)[1].rsplit(')',1)[0] if False else body('owner').split('\n(',1)[1])))
nei=list(map(int,re.findall(r'\n(\d+)(?=\n)', body('neighbour').split('\n(',1)[1])))
nc=max(own)+1
print('points',len(pts),'faces',len(faces),'owner',len(own),'nei',len(nei),'cells',nc)
def sub(a,b): return (a[0]-b[0],a[1]-b[1],a[2]-b[2])
def add(a,b): return (a[0]+b[0],a[1]+b[1],a[2]+b[2])
def cross(a,b): return (a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0])
def dot(a,b): return a[0]*b[0]+a[1]*b[1]+a[2]*b[2]
def mul(a,s): return (a[0]*s,a[1]*s,a[2]*s)
# face centres and areas (OpenFOAM primitiveMeshFaceCentresAndAreas)
fc=[];fa=[]
for f in faces:
    n=len(f)
    if n==3:
        p=[pts[i] for i in f]
        c=mul(add(add(p[0],p[1]),p[2]),1/3.0)
        a=mul(cross(sub(p[1],p[0]),sub(p[2],p[0])),0.5)
    else:
        s=(0,0,0)
        for i in f: s=add(s,pts[i])
        est=mul(s,1.0/n)
        sa=(0,0,0); sac=(0,0,0); sumA=0
        for i in range(n):
            p=pts[f[i]]; q=pts[f[(i+1)%n]]
            c3=add(add(p,q),est)
            a3=cross(sub(q,p),sub(est,p))
            sa=add(sa,a3)
            ma=math.sqrt(dot(a3,a3))*(1 if True else 1)
            # signed by projection onto total later; use mag
            sac=add(sac,mul(c3,ma)); sumA+=ma
        c=mul(sac,1.0/(3*sumA)) if sumA>1e-300 else est
        a=mul(sa,0.5)
    fc.append(c);fa.append(a)
# cells
cf=[[] for _ in range(nc)]
for i,o in enumerate(own): cf[o].append((i,1))
for i,n in enumerate(nei): cf[n].append((i,-1))
cc=[];cv=[]
for c in range(nc):
    s=(0,0,0)
    for i,_ in cf[c]: s=add(s,fc[i])
    est=mul(s,1.0/len(cf[c]))
    V=0;Vc=(0,0,0)
    for i,sg in cf[c]:
        a=mul(fa[i],sg)
        pv=dot(a,sub(fc[i],est))
        pc=add(mul(fc[i],0.75),mul(est,0.25))
        V+=pv;Vc=add(Vc,mul(pc,pv))
    V/=3.0
    cc.append(mul(Vc,1/(V*3.0)) if abs(V)>1e-300 else est); cv.append(V/1.0)
# fix: OpenFOAM: cellVols = sum(pyr3Vol)/3 ; cellCtrs = sum(pc*pyr3Vol)/sum(pyr3Vol)
zones={}
zs=body('cellZones')
for m in re.finditer(r'\n(\w+)\n\{\n\s+type cellZone;\ncellLabels\s+List<label>\s*\n(\d+)\n\(\n([\d\n]*)\)', zs):
    zones[m.group(1)]=list(map(int,m.group(3).split()))
print({k:(len(v),sum(cv[i] for i in v)) for k,v in zones.items()})
hx=zones['hx']
for axis in (0,2):
    order=sorted(hx,key=lambda i:(round(cc[i][axis],9),cc[i][1]))
    tot=sum(cv[i] for i in hx)
    best=None
    acc=0
    # cut only between distinct coordinates
    for k in range(1,len(order)):
        acc+=cv[order[k-1]]
        mism=abs(acc-(tot-acc))/acc
        if best is None or mism<best[0]: best=(mism,k,cc[order[k-1]][axis],cc[order[k]][axis],acc,tot-acc)
    print('axis',axis,'best split (mismatch (pV-sV)/pV, k, x_lo, x_hi, pV, sV):',best)
    if axis==int(sys.argv[1]) if len(sys.argv)>1 else False:
        mism,k,_,_,pv,sv=best
        A=sorted(order[:k]);B=sorted(order[k:])
        out=open('/home/user/upstream-build/gh1895/cellZones_two','w')
        txt=open(d+'cellZones').read()
        def blk(name,cells):
            return '%s\n{\n    type cellZone;\ncellLabels      List<label> \n%d\n(\n%s\n)\n;\n}\n'%(name,len(cells),'\n'.join(map(str,cells)))
        k0=txt.index('\n4\n(')
        head=txt[:k0]+'\n6\n('
        rest=txt[k0+len('\n4\n('):]
        j=rest.rindex('\n)\n')
        out.write(head+rest[:j]+'\n'+blk('hxA',A)+blk('hxB',B)+')\n\n// ************************************************************************* //\n')
        print('wrote',len(A),len(B),pv,sv,mism)
