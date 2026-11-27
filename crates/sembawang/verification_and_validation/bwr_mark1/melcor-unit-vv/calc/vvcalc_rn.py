import math as m
R_kcal=1.987e-3
Rcode=8.314462/4184.0
print("R_code_kcal",Rcode, "rel diff", Rcode/R_kcal-1)
tab={"XE":(2.0e5,63.8),"CS":(2.0e5,63.8),"BA":(2.95e5,100.2),"I2":(2.0e5,63.8),"TE":(2.0e5,63.8),
"RU":(1.62e6,152.8),"MO":(23.15,44.1),"CE":(2.67e8,188.2),"LA":(1.46e7,143.1),"UO2":(1.46e7,143.1),"CD":(5.95e3,70.8),"AG":(5.95e3,70.8)}
print("== CORSOR-M per min, manual R")
for T in (1173.0,1500.0,2000.0,2400.0,2500.0):
    print(T, {k:"%.6e"%(a*m.exp(-q/(R_kcal*T))) for k,(a,q) in tab.items()})
for T in (1000.0,2400.0):
    a,q=tab["CS"]; man=a/60*m.exp(-q/(R_kcal*T)); code=a/60*m.exp(-q*1000*4.184/(8.314462*T))
    print("CS /s",T,"manual %.6e code %.6e rel %.3e"%(man,code,code/man-1))
a,q=tab["UO2"]; T=2500.0
man=a/60*m.exp(-q/(R_kcal*T)); code=a/60*m.exp(-q*1000*4.184/(8.314462*T)); print("UO2 2500 /s manual %.6e code %.6e rel %.3e"%(man,code,code/man-1))
# half time at 2400
k=2e5/60*m.exp(-63.8/(R_kcal*2400)); print("t_half 2400K s",m.log(2)/k)
print("== CORSOR A exp(BT)")
cs=[(900,7.53e-12,0.0142),(1400,2.02e-7,0.00667),(2200,1.74e-5,0.00460)]
def corsor(T,rows):
    if T<rows[0][0]: return 0.0
    r=[x for x in rows if x[0]<=T][-1]; return r[1]*m.exp(r[2]*T)
for T in (899.0,900.0,1399.0,1400.0,2000.0,2199.0,2200.0,2400.0):
    print(T,"%.6e"%corsor(T,cs))
# same with T in C (i.e. use T_K-273.15 and ranges in C)
for TK in (1673.15,2473.15):
    print("degC reading TK",TK,"%.6e"%corsor(TK-273.15,cs), "K reading %.6e"%corsor(TK,cs))
print("== Booth")
D0=1e-6;Q=3.814e5;a=6e-6;R=8.314462618
for T in (1500.,2000.,2500.):
    D=D0*m.exp(-Q/(R*T)); print(T,"D %.6e D/a2 %.6e"%(D,D/a/a))
def booth(x):
    return 6*m.sqrt(x/m.pi)-3*x if x<1/m.pi**2 else 1-6/m.pi**2*m.exp(-m.pi**2*x)
x0=1/m.pi**2
print("at 1/pi2: lo %.6f hi %.6f"%(6*m.sqrt(x0/m.pi)-3*x0, 1-6/m.pi**2*m.exp(-1)))
for x in (1e-4,1e-2,0.05,0.2,1.0): print(x,"%.6f"%booth(x))
print("== S/V", 2.0*800/422.5)
