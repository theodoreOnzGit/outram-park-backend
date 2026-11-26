import math
cols={"CORCON-basalt":dict(SiO2=54.84,TiO2=1.05,MgO=6.16,CaO=8.82,Na2O=1.80,K2O=5.39,Fe2O3=6.26,Al2O3=8.32,CO2=1.50,H2OCHEM=2.00,H2OEVAP=3.86),
"CORCON-L/CS":dict(SiO2=35.80,TiO2=0.18,MnO=0.03,MgO=0.48,CaO=31.30,Na2O=0.082,K2O=1.22,Fe2O3=1.44,Al2O3=3.60,Cr2O3=0.014,CO2=21.15,H2OCHEM=2.00,H2OEVAP=2.70),
"CORCON-GSEUS":dict(SiO2=3.60,TiO2=0.12,MnO=0.01,MgO=5.67,CaO=45.40,Na2O=0.078,K2O=0.68,Fe2O3=1.20,Al2O3=1.60,Cr2O3=0.004,CO2=35.698,H2OCHEM=2.00,H2OEVAP=3.94),
"BASALT":dict(SiO2=65.0,Al2O3=20.0,CaCO3=3.0,CaOH2=8.0,H2OEVAP=4.0),
"LIMESTONE/CS":dict(SiO2=36.0,Al2O3=5.0,CaCO3=48.0,CaOH2=8.0,H2OEVAP=3.0),
"CRBR":dict(SiO2=4.0,Al2O3=3.0,CaCO3=81.0,CaOH2=8.0,H2OEVAP=4.0)}
M=dict(H2O=0.0180153,CO2=0.0440095,CaCO3=0.1000869,CaOH2=0.0740927,CaO=0.0560774)
for k,c in cols.items():
    s=sum(c.values())
    co2=c.get("CO2",0)/100+c.get("CaCO3",0)/100*M["CO2"]/M["CaCO3"]
    h2o=(c.get("H2OCHEM",0)+c.get("H2OEVAP",0))/100+c.get("CaOH2",0)/100*M["H2O"]/M["CaOH2"]
    n=co2/M["CO2"]+h2o/M["H2O"]
    print(f"{k:14s} sum={s:.4f} CO2={co2:.6f} H2O={h2o:.6f} gas mol/kg={n:.6f} H2+CO mol/kg if all reduced={n:.6f} Zr kg/kg conc={0.5*n*0.091224:.6f}")
# CAV-03 ablation
q=1000*21.2*(2400-1450); ha=1.147e6+0.087e6+0.121e6+0.061e6+0.4e6
print("ha",ha,"mdot",repr(q/ha),"q",q)
print("ablation velocity m/s per MW/m2 at ha=ha, rho 2340:", repr(1e6/(ha*2340)), "cm/h",1e6/(ha*2340)*3.6e5)
print("bound water heat",0.02/0.0180153*109e3, "co2 heat",0.015/0.0440095*178e3, "sensible", 1000*(1450-303))
# slag
print("slag", 0.41/(1+0.41))
# mixing analytic
def md(t,md0,me,tau): return md0+(me*tau-md0)*(1-math.exp(-t/tau))
L=0.5; vs=0.01; tau=L/vs; me=2.0; md0=10.0
for t in [0,10,50,1e4]: print("MD",t,repr(md(t,md0,me,tau)))
print("dMnet dt=10", repr((me*tau-md0)*(1-math.exp(-10/tau))))
# euler check
m=md0;dt=1e-4
for i in range(int(10/dt)): m+= (me - m/tau)*dt
print("euler",m)
