import sys
def load(p):
    d={}
    order=[]
    for l in open(p):
        w=l.split()
        if len(w)<4 or w[0]!='D': continue
        k=(int(w[1]),w[2])
        d[k]=[float(x) for x in w[3:]]
        order.append(k)
    return d,order
up,order=load(sys.argv[1]); po,_=load(sys.argv[2])
maxcall=int(sys.argv[3]) if len(sys.argv)>3 else 3
for k in order:
    if k[0]>maxcall: break
    if k not in po: print(k,'missing in port'); continue
    a=up[k]; b=po[k]
    n=min(len(a),len(b))
    sc=max([abs(x) for x in a]+[1e-300])
    d=max(abs(a[i]-b[i]) for i in range(n))
    print(k, len(a), len(b), 'maxabs %.2e rel_to_max %.2e'%(d,d/sc))
