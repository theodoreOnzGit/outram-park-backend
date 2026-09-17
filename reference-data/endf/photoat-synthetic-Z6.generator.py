import math
MAT=600; ZA=6000.0; AWR=11.9078; Z=6.0
def ff(x):
    # ENDF 11-char float: d.dddddd+ee
    if x==0.0: return " 0.000000+0"
    s='-' if x<0 else ' '
    x=abs(x); e=int(math.floor(math.log10(x))); m=x/10**e
    if round(m,6)>=10.0: m/=10; e+=1
    return f"{s}{m:.6f}{e:+d}" if abs(e)<10 else f"{s}{m:.5f}{e:+d}"
def fi(n): return f"{n:11d}"
lines=[]; seq=[0]
def emit(body, mf, mt):
    seq[0]+=1
    lines.append(f"{body:<66s}{MAT:4d}{mf:2d}{mt:3d}{seq[0]:5d}")
def cont(c1,c2,l1,l2,n1,n2,mf,mt): emit(ff(c1)+ff(c2)+fi(l1)+fi(l2)+fi(n1)+fi(n2),mf,mt)
def text(t,mf,mt): emit(t[:66],mf,mt)
def send(mf): seq[0]=0; lines.append(f"{' 0.000000+0 0.000000+0          0          0          0          0':66s}{MAT:4d}{mf:2d}{0:3d}{99999:5d}"); seq[0]=0
def fend(): lines.append(f"{' 0.000000+0 0.000000+0          0          0          0          0':66s}{MAT:4d}{0:2d}{0:3d}{0:5d}"); seq[0]=0
def mend(): lines.append(f"{' 0.000000+0 0.000000+0          0          0          0          0':66s}{0:4d}{0:2d}{0:3d}{0:5d}")
def tend(): lines.append(f"{' 0.000000+0 0.000000+0          0          0          0          0':66s}{-1:4d}{0:2d}{0:3d}{0:5d}")
def tab1(c1,c2,l1,l2,pairs,intlaw,mf,mt):
    cont(c1,c2,l1,l2,1,len(pairs),mf,mt)
    emit(fi(len(pairs))+fi(intlaw),mf,mt)
    flat=[v for p in pairs for v in p]
    for i in range(0,len(flat),6):
        emit(''.join(ff(v) for v in flat[i:i+6]),mf,mt)
# ---- data ----
def egrid():
    g=set()
    for d in range(3,12):
        for m in (1.0,1.5,2.0,3.0,5.0,7.0): 
            v=m*10**d
            if v<=1e11: g.add(v)
    g.update([1.022e6, 1.05e6, 1.1e6, 1.2e6])
    return sorted(g)
E=egrid()
def s_coh(e): return 2.4/(1+(e/3.0e4)**2)+1e-9
def s_inc(e): k=e/5.11e5; return 3.99/(1+k)**0.9*(1-0.3*math.exp(-e/1e4))+1e-9
def s_pe(e): return max(4.0e3*(e/1e3)**-3, 1e-8)
def s_pair(e): return 0.0 if e<=1.022e6 else 0.2*math.log(e/1.022e6)
coh=[(e,s_coh(e)) for e in E]; inc=[(e,s_inc(e)) for e in E]; pe=[(e,s_pe(e)) for e in E]
pair=[(e,s_pair(e)) for e in E if e>=1.022e6]
tot=[(e,s_coh(e)+s_inc(e)+s_pe(e)+s_pair(e)) for e in E]
xg=[0.0,0.005,0.01,0.02,0.05,0.1,0.2,0.3,0.5,0.7,1.0,1.5,2.0,3.0,5.0,7.0,10.0,20.0,50.0,100.0,1e3,1e4,1e6,1e9]
F=[(x, Z/(1+(x/0.6)**2)**2) for x in xg]
S=[(x, Z*(1-1/(1+(x/0.5)**2))) for x in xg]
# ---- tape ----
lines.append(f"{'synthetic Z=6 photoatomic test tape (not an evaluation)':<66s}{1:4d}{0:2d}{0:3d}{0:5d}")
cont(ZA,AWR,0,0,0,0,1,451)
cont(0.0,0.0,0,0,0,6,1,451)
cont(0.0,1.0e11,0,0,3,8,1,451)
cont(0.0,0.0,0,0,2,8,1,451)
text(" 6-C-0 SYNTHETIC  PHOTOATOMIC TEST DATA          OUTRAM-PARK",1,451)
text(" NOT AN EVALUATION: analytic shapes for NJOY2016 oracle runs.",1,451)
dirs=[(1,451,6),(23,501,3+ (len(tot)*2+5)//6),(23,502,0),(23,504,0),(23,516,0),(23,522,0),(27,502,0),(27,504,0)]
for mf,mt,nc in dirs: emit(fi(0).replace('0',' ')*2+fi(mf)+fi(mt)+fi(nc)+fi(0),1,451)
send(1); fend()
for mt,pairs in ((501,tot),(502,coh),(504,inc),(516,pair),(522,pe)):
    cont(ZA,AWR,0,0,0,0,23,mt); tab1(0.0,0.0,0,0,pairs,2,23,mt); send(23)
fend()
for mt,pairs in ((502,F),(504,S)):
    cont(ZA,AWR,0,0,0,0,27,mt); tab1(0.0,Z,0,0,pairs,2,27,mt); send(27)
fend(); mend(); tend()
open('photoat-synthetic-Z6.endf','w').write('\n'.join(lines)+'\n')
print(len(lines),"lines;", len(E),"energies;", len(xg),"x points")
