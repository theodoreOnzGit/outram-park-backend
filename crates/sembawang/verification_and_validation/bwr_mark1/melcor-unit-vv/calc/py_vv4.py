import math
T=5.05e7; psi=0.713
G=lambda t:1+(3.24e-6+5.23e-10*t)*T**0.4*psi
print("DCH01 G(0)",repr(G(0.0)),"G(1e4-)",repr(G(1e4-1e-9)),"Gmax(1e4)=1.064")
print("DCH01 T^0.4",repr(T**0.4))
# Gmax interpolation between 1e4 (1.064) and 1.5e4 (1.074) at 1.2e4
t=1.2e4; w=math.log(t/1e4)/math.log(1.5)
print("DCH01 Gmax(1.2e4) lin-in-ln t",repr(1.064+w*(0.010)),"log-log",repr(math.exp(math.log(1.064)+w*math.log(1.074/1.064))))
# actinides
R=0.526;EU=0.474;EN=0.419;l1=4.91e-4;l2=3.41e-6
def FU(t): return EU*R*(1-math.exp(-l1*T))*math.exp(-l1*t)
def FN(t): return EN*R*(l1*(1-math.exp(-l2*T))*math.exp(-l2*t)/(l1-l2) - l2*(1-math.exp(-l1*T))*math.exp(-l1*t)/(l1-l2))
for t in [0.0,1e3,1.8e4,1e5]:
    print("DCH02 t",t,"FU",repr(FU(t)),"FNp",repr(FN(t)),"sum MeV/fission",repr(FU(t)+FN(t)))
# fraction of rated: sum_i P_i/Q_i * (FU+FN) / P ; shares
s235=2.316/3.578; s239=1.1114/3.578; s238=1-s235-s239
Qinv=s235/199.0+s239/210.2+s238/199.3
print("DCH02 mean 1/Q",repr(Qinv),"Qeff",repr(1/Qinv))
for t in [0.0,1.8e4]: print("DCH02 Pd/P0 at",t,repr(Qinv*(FU(t)+FN(t))))
# F(t,inf) table for U235 (subset)
tab={1e4:(1.908,1.727,1.777),1.5e4:(1.685,1.548,1.578),4e7:(1.457e-1,1.302e-1,1.299e-1),6e7:(1.308e-1,1.099e-1,1.113e-1),1e2:(6.198,5.685,6.217),1.5e2:(5.696,5.262,5.621),2e7:(1.846e-1,1.728e-1,1.717e-1)}
def interp(t,j,mode):
    ks=sorted(tab); lo=max(k for k in ks if k<=t); hi=min(k for k in ks if k>=t)
    if lo==hi: return tab[lo][j]
    w=math.log(t/lo)/math.log(hi/lo)
    a,b=tab[lo][j],tab[hi][j]
    return math.exp(math.log(a)+w*math.log(b/a)) if mode=="loglog" else a+w*(b-a)
for mode in ["loglog","linlog"]:
    t=1e4
    F=[interp(t,j,mode)-interp(t+T,j,mode) for j in range(3)]
    P_over_P0=G(1e4-1e-9)*(s235*F[0]/199.0+s239*F[1]/210.2+s238*F[2]/199.3)
    print("DCH03",mode,"F(1e4,T)",[repr(x) for x in F],"P_FP/P0 with G(1e4-)",repr(P_over_P0),"with Gmax",repr(P_over_P0/G(1e4-1e-9)*1.064))
    print("   total with actinides (Gmax)",repr(P_over_P0/G(1e4-1e-9)*1.064+Qinv*(FU(1e4)+FN(1e4))))
