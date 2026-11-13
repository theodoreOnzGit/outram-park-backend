# Pool route-4 seeds per arm and compute N-3 coefficients (per °F) and N-1 pcm.
import csv, math, sys, collections
S='/tmp/claude-0/-home-user-outram-park-backend/ffb0ef1d-d531-5faf-a6d8-9c58de41ccae/scratchpad/n1/'
def pool(rows):
    k=[float(r['k']) for r in rows]; n=len(k); m=sum(k)/n
    sd=math.sqrt(sum((x-m)**2 for x in k)/(n-1)) if n>1 else float('nan')
    sem_int=math.sqrt(sum(float(r['k_std'])**2 for r in rows))/n   # from internal sigmas
    sem=sd/math.sqrt(n) if n>1 else sem_int
    return m, sem, sem_int, n
arms=collections.defaultdict(list)
for f in ['keff.csv','n3.csv']:
    try:
        for r in csv.DictReader(open(S+f)):
            a=r['arm'].replace('@1181F','@salt1181F/graphite1181F'); arms[a].append(r)
    except FileNotFoundError: pass
P={}
for a,rows in sorted(arms.items()):
    m,sem,si,n=pool(rows); P[a]=(m,max(sem,si) if n<3 else sem,si,n)
    print(f"{a:45s} n={n} k={m:.5f} sem(seeds)={sem:.5f} sem(internal)={si:.5f}")
ref=1/(1-0.0008)
a='mo-omitted@salt1181F/graphite1181F'
if a in P: m,s,_,n=P[a]; print(f"N-1 arm A: {1e5*(m-ref)/ref:+.0f} +/- {1e5*s/ref:.0f} pcm (n={n})")
def rho(k): return 1-1/k
def coef(hi,lo,dT=100.0):
    if hi in P and lo in P:
        (k2,s2,_,_),(k1,s1,_,_)=P[hi],P[lo]
        a=(rho(k2)-rho(k1))/dT; s=math.hypot(s2/k2**2,s1/k1**2)/dT
        return a,s
A='mo-omitted@salt1131F/graphite1131F'; B='mo-omitted@salt1231F/graphite1231F'
C='mo-omitted@salt1231F/graphite1131F'; D='mo-omitted@salt1131F/graphite1231F'
for name,hi in [('isothermal',B),('fuel',C),('graphite',D)]:
    r=coef(hi,A)
    if r: print(f"alpha_{name} = {r[0]*1e5:+.2f} +/- {r[1]*1e5:.2f} e-5 /F")
