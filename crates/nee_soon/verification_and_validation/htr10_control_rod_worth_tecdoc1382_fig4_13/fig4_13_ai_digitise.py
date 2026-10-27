# AI DIGITISATION script (Claude Code agent, 2026-10-23) for IAEA-TECDOC-1382 Fig. 4.13.
# Untrusted until reviewed; see fig4_13_experiment_AI_digitised.md for method and checks.
# Usage: python3 -I fig4_13_ai_digitise.py <embedded raster .jpg> <out.json> <out.csv>
# Needs numpy and Pillow. The raster is not committed (IAEA figure).
import sys, json
from PIL import Image
import numpy as np
from collections import deque
im=np.asarray(Image.open(sys.argv[1]).convert('RGB')).astype(float)
R,G,B=im[...,0],im[...,1],im[...,2]; H,W=R.shape; gray=im.mean(-1)
# --- calibration (pixel centres measured on the embedded 304x192 image) ---
ticks=np.array([52.0,60,69,77,85,93.45,101.49,110,118,126,134,143,151,159,168,176,184,192,200,209,217,225,234,242,250,259,267,275,284,292])
tv=np.arange(10,301,10.0)
ax,bx=np.polyfit(ticks,tv,1)           # cm = ax*col + bx
grid_r=np.array([58.25,76.95,94.5,112.11,129.92,147.42,164.78]); grid_v=np.array([0.010,0.008,0.006,0.004,0.002,0.000,-0.002])
ay,by=np.polyfit(grid_r,grid_v,1)      # value = ay*row + by
res_x=np.std(ticks*ax+bx-tv); res_y=np.std(grid_r*ay+by-grid_v)
legend=np.zeros((H,W),bool); legend[5:60,133:280]=True
plot=np.zeros((H,W),bool); plot[3:164,45:292]=True
mag=(R>170)&(G<120)&(R-G>90)&plot&~legend
navy=(B>R+12)&(B>G+5)&(gray<140)&plot&~legend
# drop single-row navy streaks lying on gridlines (gridline/antialias artefacts)
for gr in grid_r:
    r=int(round(gr))
    for rr in (r-1,r,r+1):
        thin=navy[rr]&~navy[rr-2]&~navy[rr+2]
        navy[rr]&=~thin
def comps(m):
    seen=np.zeros_like(m); out=[]
    for r0,c0 in zip(*np.nonzero(m)):
        if seen[r0,c0]: continue
        q=deque([(r0,c0)]); seen[r0,c0]=True; px=[]
        while q:
            r,c=q.popleft(); px.append((r,c))
            for dr in (-1,0,1):
                for dc in (-1,0,1):
                    rr,cc=r+dr,c+dc
                    if 0<=rr<H and 0<=cc<W and m[rr,cc] and not seen[rr,cc]:
                        seen[rr,cc]=True; q.append((rr,cc))
        a=np.array(px); out.append(a)
    return out
def tocm(c): return ax*c+bx
def toval(r): return ay*r+by
result={'calibration':{'by':by,'cm_per_px':ax,'x0_col':-bx/ax,'value_per_px':ay,'tick_fit_resid_cm':res_x,'grid_fit_resid':res_y}}
for name,m in (('experiment',mag),('calculation',navy)):
    cs=comps(m); pts=[]
    for a in cs:
        n=len(a)
        if n<6: continue
        rr,cc=a[:,0],a[:,1]
        pts.append({'x_cm':tocm(cc.mean()),'worth':toval(rr.mean()),'npx':int(n),
                    'x_span_cm':[tocm(cc.min()),tocm(cc.max())],'worth_span':[toval(rr.max()),toval(rr.min())]})
    pts.sort(key=lambda p:p['x_cm'])
    # column trace (mean row per column)
    trace=[]
    for c in range(W):
        rows=np.nonzero(m[:,c])[0]
        if len(rows): trace.append((tocm(c),toval(rows.mean()),toval(rows.max()),toval(rows.min())))
    result[name]={'components':pts,'trace':trace}
json.dump(result,open(sys.argv[2],'w'),indent=1)
print(json.dumps(result['calibration'],indent=1))
for name in ('experiment','calculation'):
    print(name,len(result[name]['components']),'components')
    for p in result[name]['components']:
        print('  x=%6.1f  w=%.5f  npx=%3d  xspan=%5.1f..%5.1f'%(p['x_cm'],p['worth'],p['npx'],*p['x_span_cm']))

# ---- second pass: morphological opening (3x3) strips the 1-2 px connecting lines, leaving marker bodies
def shift(m,dr,dc):
    o=np.zeros_like(m); H,W=m.shape
    o[max(dr,0):H+min(dr,0), max(dc,0):W+min(dc,0)] = m[max(-dr,0):H+min(-dr,0), max(-dc,0):W+min(-dc,0)]
    return o
def erode(m):
    o=m.copy()
    for dr in (-1,0,1):
        for dc in (-1,0,1): o&=shift(m,dr,dc)
    return o
def dilate(m):
    o=m.copy()
    for dr in (-1,0,1):
        for dc in (-1,0,1): o|=shift(m,dr,dc)
    return o
for name,m in (('experiment',mag),('calculation',navy)):
    op=erode(m)
    cs=comps(op); pts=[]
    for a in cs:
        rr,cc=a[:,0],a[:,1]
        pts.append({'x_cm':tocm(cc.mean()),'worth':toval(rr.mean()),'core_px':len(a),'w_cols':int(cc.max()-cc.min()+1),'h_rows':int(rr.max()-rr.min()+1)})
    pts.sort(key=lambda p:p['x_cm']); result[name]['cores']=pts
    print(name,'cores')
    for p in pts: print('  x=%6.1f  w=%.5f core=%3d  %dx%d'%(p['x_cm'],p['worth'],p['core_px'],p['w_cols'],p['h_rows']))
json.dump(result,open(sys.argv[2],'w'),indent=1)

# ---- third pass: split merged marker cores (2-means), build the final point list
def kmeans2(a):
    a=a.astype(float); c0,c1=a[a[:,1].argmin()],a[a[:,1].argmax()]
    for _ in range(20):
        lab=((a-c0)**2).sum(1)>((a-c1)**2).sum(1)
        c0,c1=a[~lab].mean(0),a[lab].mean(0)
    return [a[~lab],a[lab]]
final=[]
for name,m in (('experiment',mag),('calculation',navy)):
    for a in comps(erode(m)):
        cols=a[:,1].max()-a[:,1].min()+1
        if name=='experiment' and cols>20:   # dense overlapping chain: not resolvable into markers
            continue
        parts=kmeans2(a) if (cols>=9 or (a[:,0].max()-a[:,0].min()+1)>=9) else [a]
        for p in parts:
            if len(p)<3: continue
            final.append((name,'marker',tocm(p[:,1].mean()),toval(p[:,0].mean()),len(p)))
    # column trace for the chain region (experiment) every 2 px
    if name=='experiment':
        for c in range(int(round(41.6/ax+(-bx/ax))),int(round(160/ax+(-bx/ax)))+1):
            rows=np.nonzero(m[:,c])[0]
            if len(rows): final.append((name,'trace',tocm(c),toval(rows.mean()),len(rows)))
final.sort(key=lambda t:(t[0],t[1],t[2]))
with open(sys.argv[3],'w') as f:
    f.write('series,kind,elevation_cm,worth_dk_k,support_px\n')
    for s,k,x,w,n in final: f.write('%s,%s,%.1f,%.5f,%d\n'%(s,k,x,w,n))
