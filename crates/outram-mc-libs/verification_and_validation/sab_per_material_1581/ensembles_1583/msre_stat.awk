{n++; o[n]=$2; w[n]=$4; so+=$2; sw+=$4}
END{mo=so/n; mw=sw/n; for(i=1;i<=n;i++){vo+=(o[i]-mo)^2; vw+=(w[i]-mw)^2} vo/=n-1; vw/=n-1; eo=sqrt(vo/n); ew=sqrt(vw/n); sg=sqrt(eo^2+ew^2);
printf "N=%d old mean %.5f sd %.0f sem %.0f | new mean %.5f sd %.0f sem %.0f | new-old %+.0f +/- %.0f pcm z %+.2f\n",n,mo,sqrt(vo)*1e5,eo*1e5,mw,sqrt(vw)*1e5,ew*1e5,(mw-mo)*1e5,sg*1e5,(mw-mo)/sg}
