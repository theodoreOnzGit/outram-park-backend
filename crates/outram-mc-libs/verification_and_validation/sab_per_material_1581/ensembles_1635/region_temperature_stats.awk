{ if ($3!=$9) mm++; if ($3!=$5) oh++; a[NR]=$5; b[NR]=$11; sa+=$5; sb+=$11; n++ }
END { ma=sa/n; mb=sb/n; for(i=1;i<=n;i++){va+=(a[i]-ma)^2; vb+=(b[i]-mb)^2}; va/=(n-1); vb/=(n-1); sea=sqrt(va/n); seb=sqrt(vb/n); d=mb-ma; se=sqrt(sea^2+seb^2);
printf "N %d; model col old!=new: %d; old model!=hand: %d\nold mean %.6f sd %.6f sem %.6f\nnew mean %.6f sd %.6f sem %.6f\ndiff %.6f (%.0f pcm) +/- %.6f (%.0f pcm) z %.2f\n", n, mm+0, oh+0, ma, sqrt(va), sea, mb, sqrt(vb), seb, d, d*1e5, se, se*1e5, d/se }
