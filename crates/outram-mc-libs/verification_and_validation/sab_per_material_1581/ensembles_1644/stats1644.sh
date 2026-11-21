#!/bin/bash
# gh:#1644 statistics (awk only). Reads the raw outputs next to it.
R=/tmp/claude-0/-home-user-outram-park-backend/e2a0dbce-776f-5340-8a16-e227c5d4dc69/scratchpad/r1644
for f in new_1-8 new_9-16 new_17-24; do
  grep -E "ABLATED" $R/$f.out | awk '{print $2+0, $4}'
done > $R/new_abl.txt
echo "NEW per-seed ablated k (seed k):"; cat $R/new_abl.txt | paste -sd' ' | fold -w 100
awk '
{ s[NR]=$1; k[NR]=($2-1)*1e5 }
function stat(a,b,  i,n,m,v){n=b-a+1;m=0;for(i=a;i<=b;i++)m+=k[i];m/=n;v=0;for(i=a;i<=b;i++)v+=(k[i]-m)^2;v/=n-1;SM=m;SD=sqrt(v);SE=sqrt(v/n)}
END{
 for(b=0;b<3;b++){stat(b*8+1,b*8+8);printf "NEW block %d-%d: mean %+.1f pcm sd %.0f sem %.0f\n",s[b*8+1],s[b*8+8],SM,SD,SE}
 stat(1,24);printf "NEW pooled 1-24: mean %+.1f sd %.0f sem %.0f\n",SM,SD,SE
}' $R/new_abl.txt
# OLD: summary-line block means (pcm above 1.0) and sem from the driver, unpatched binary
awk 'BEGIN{
 m[1]=159;e[1]=43; m[2]=441;e[2]=46; m[3]=157;e[3]=72
 for(i=1;i<=3;i++){w=1/(e[i]^2);sw+=w;swm+=w*m[i]}
 mm=swm/sw; for(i=1;i<=3;i++){c+=((m[i]-mm)/e[i])^2; printf "OLD block %d mean %d sem %d z vs weighted mean %+.2f\n",i,m[i],e[i],(m[i]-mm)/e[i]}
 printf "OLD weighted mean %.1f, chi2 %.1f (2 dof)\n",mm,c
 a=(159+441+157)/3; se=sqrt(43^2+46^2+72^2)/3; printf "OLD unweighted pooled 1-24 mean %.1f sem %.1f\n",a,se
 # 9-16 vs the other two blocks pooled
 o=(159+157)/2; so=sqrt(43^2+72^2)/2; d=441-o; sd=sqrt(46^2+so^2); printf "OLD 9-16 minus mean(1-8,17-24): %+.1f +/- %.1f z %+.2f\n",d,sd,d/sd
}'
