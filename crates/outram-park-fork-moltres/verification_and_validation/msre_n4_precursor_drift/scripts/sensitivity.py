# N-4 (gh:#1130) digitisation sensitivity of the TM-1626 line model.
# Independent re-implementation of the test's exact integral (midpoint source),
# varying the inputs read from TM-1626 Figs. 4-5. Run: python3 -I sensitivity.py
import math
T1=[(2.23e-4,0.0124),(14.57e-4,0.0305),(13.07e-4,0.1114),(26.28e-4,0.3013),(7.66e-4,1.140),(2.80e-4,3.010)]
F=[(-22,.015,.045),(-20,.015,.055),(-10,.035,.13),(-5,.07,.19),(0,.12,.26),(10,.33,.37),(20,.5,.5),(30,.62,.62),(40,.73,.73),(50,.83,.83),(60,.9,.9),(70,.95,.95),(80,.985,.985),(90,.99,.99),(100,.97,.97),(110,.925,.925),(120,.86,.86),(130,.78,.78),(140,.68,.69),(150,.56,.585),(160,.39,.44),(165,.25,.38),(170,.11,.34),(180,.05,.17),(185,.025,.12),(190,.008,.07)]
def run(tau=(3.8,9.4,3.9,8.1),hc=168.0,dp=0.0,da=0.0,da_pl=0.0,m=50400):
    T=sum(tau)
    def fig(z):
        for (z0,p0,a0),(z1,p1,a1) in zip(F,F[1:]):
            if z<=z1:
                f=(z-z0)/(z1-z0); p=p0+f*(p1-p0); a=a0+f*(a1-a0); break
        else: p,a=F[-1][1],F[-1][2]
        pl = z<0 or z>hc
        return max(p+dp,0), max(a+da+(da_pl if pl else 0),0)
    def sa(t):
        t1,t2,t3=tau[0],tau[0]+tau[1],tau[0]+tau[1]+tau[2]
        if t<t1: z=-22+22*t/tau[0]
        elif t<t2: z=hc*(t-t1)/tau[1]
        elif t<t3: z=hc+(190-hc)*(t-t2)/tau[2]
        else: return 0,0
        return fig(z)
    dt=T/m; S=[sa((j+.5)*dt) for j in range(m)]
    den=sum(p*a for p,a in S)
    num=0
    for b,l in T1:
        e=math.exp(-l*dt)
        # c_{j+1} = e c_j + b p_j (1-e)/l (source constant per step);
        # periodic closure: one loop from c = 0 gives c0, then c* = c0/(1-e^m).
        A=e**m
        c0=0
        for p,a in S: c0=e*c0+b*p*(1-e)/l
        cp=c0/(1-A)
        c=cp; acc=0
        for p,a in S:
            c=e*c+b*p*(1-e)/l; acc+=l*c*a
        num+=acc
    beta=sum(x[0] for x in T1)
    return 100*(beta-num/den)
print("base", run())
print("hc166", run(hc=166), "hc170", run(hc=170))
print("adj+0.02 plenums", run(da_pl=0.02), "adj-0.02 plenums", run(da_pl=-0.02))
print("P+0.01", run(dp=0.01))
print("no plenum importance", run(da_pl=-1))
