#!/bin/bash
# gh:#1887 (README round 47): the PHYSICAL two-group slab data set of the SP3 discontinuity-factor comparison.
#   make_r47_data.sh data <name> <adjust true|false> [<integrals file>]   -> cases/df_slab/nuclearData.<name>
#   make_r47_data.sh integrals <run dir> <field: flux|fluxStar>           -> prints the zone fractions (reflector coreLow follower
#                                                                          controlRod; groups 0 and 1), one line per zone
# CROSS SECTIONS: the IAEA 2-D PWR benchmark (Argonne Code Center Benchmark Problem Book, ANL-7416 Suppl. 2, 1977, problem
# 11-A2; two groups, chi = (1, 0), no up-scatter), values in cm and cm^-1 as I recalled them (NOT re-checked against the
# primary document in this session):
#   fuel 1          D 1.5 0.4   Sigma_a 0.01 0.080   nuSigma_f 0 0.135   Sigma_s12 0.02       -> coreLow
#   fuel 2          D 1.5 0.4   Sigma_a 0.01 0.085   nuSigma_f 0 0.135   Sigma_s12 0.02       -> follower
#   fuel 2 + rod    D 1.5 0.4   Sigma_a 0.01 0.130   nuSigma_f 0 0.135   Sigma_s12 0.02       -> controlRod
#   reflector       D 2.0 0.3   Sigma_a 0.00 0.010   nuSigma_f 0 0       Sigma_s12 0.04       -> reflector
# Converted to m (D / 100) and m^-1 (Sigma x 100). GeN-Foam's removal is Sigma_r,g = Sigma_a,g + Sigma_s,g->g' (no in-group
# term). SP3 also needs the in-group scattering (second-moment equation: total = sigmaRemoval + sigmaFromTo[g][g]), which
# the benchmark does not tabulate: DERIVED from the diffusion coefficient with isotropic scattering, D = 1/(3 Sigma_t), so
# Sigma_s,gg = 1/(3 D_g) - Sigma_r,g (all positive: 0.192, 0.753 fuel 1; 0.127, 1.101 reflector). nu = 2.5 and one energy
# unit per fission are assumed for sigmaPow (power is renormalised to pTarget, so only the group ratio matters). Delayed
# data, 1/v and the chi are those of the round-39 data set (inputs of both codes; 1/v is not used in an eigenvalue run).
HERE="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$HERE/cases/df_slab"

# zone <name> D1 D2 Sa1 Sa2 nSf1 nSf2 Ss12 integralFlux0 integralFlux1 dfAdjust
zone() {
awk -v md="${MD:-1}" -v mn="${MN:-1}" -v mr="${MR:-1}" -v ms="${MS:-1}" -v n="$1" -v D1="$2" -v D2="$3" -v a1="$4" -v a2="$5" -v f1="$6" -v f2="$7" -v s12="$8" -v i0="$9" -v i1="${10}" -v adj="${11}" 'BEGIN{
  D1*=md; D2*=md; f1*=mn; f2*=mn; r1=(a1+s12)*mr; r2=a2*mr; s12*=ms; ss1=1/(3*D1)-r1; ss2=1/(3*D2)-r2;
  printf "            %s\n            {\n", n;
  printf "                fuelFraction    1;\n                dfAdjust        %s;\n", adj;
  printf "                D               nonuniform List<scalar> 2 (%.10g %.10g);\n", D1/100, D2/100;
  printf "                nuSigmaEff      nonuniform List<scalar> 2 (%.10g %.10g);\n", f1*100, f2*100;
  printf "                sigmaPow        nonuniform List<scalar> 2 (%.10g %.10g);\n", f1*100/2.5, f2*100/2.5;
  printf "                sigmaRemoval    nonuniform List<scalar> 2 (%.10g %.10g);\n", r1*100, r2*100;
  printf "                scatteringMatrixP0 2 2 ( ( %.10g %.10g ) ( 0 %.10g ) );\n", ss1*100, s12*100, ss2*100;
  printf "                chiPrompt       nonuniform List<scalar> 2 (1 0);\n                chiDelayed      nonuniform List<scalar> 2 (1 0);\n";
  printf "                IV              nonuniform List<scalar> 2 (7.1e-08 4.5e-04);\n";
  printf "                discFactor      nonuniform List<scalar> 2 (1 1);\n";
  printf "                integralFlux    nonuniform List<scalar> 2 (%s %s);\n", i0, i1;
  printf "                lambda          nonuniform List<scalar> 6 (0.0125371 0.0300828 0.109879 0.325484 1.3036 9.51817);\n";
  printf "                Beta            nonuniform List<scalar> 6 (7.2315e-05 0.000609661 0.000471181 0.00118907 0.000445487 9.58515e-05);\n";
  printf "            }\n" }'
}

case "$1" in
data)
  NAME=$2; ADJ=$3; INT=$4
  # integrals file: four lines "i0 i1" in the order reflector coreLow follower controlRod
  if [ -n "$INT" ]; then mapfile -t L < "$INT"; else L=("1 1" "1 1" "1 1" "1 1"); fi
  {
  echo 'FoamFile { version 2.0; format ascii; class dictionary; object nuclearData; }'
  echo "// gh:#1887 round 47: IAEA 2-D benchmark cross sections, slab zones; variant $NAME (see scripts/make_r47_data.sh)"
  echo 'fastNeutrons    true;'
  echo "adjustDiscFactors $ADJ;"
  echo; echo 'useGivenDiscFactors false;'; echo 'energyGroups    2;'; echo 'precGroups      6;'; echo 'xsVariables {}'
  echo 'states'; echo '('; echo '    reference'; echo '    {'; echo '        zones'; echo '        ('
  zone reflector 2.0 0.3 0.0 0.01 0 0 0.04 ${L[0]} true
  zone coreLow 1.5 0.4 0.01 0.080 0 0.135 0.02 ${L[1]} true
  zone follower 1.5 0.4 0.01 0.085 0 0.135 0.02 ${L[2]} true
  zone controlRod 1.5 0.4 0.01 0.130 0 0.135 0.02 ${L[3]} true
  echo '        );'; echo '    }'; echo ');'
  } > "$OUT/nuclearData.$NAME"
  ;;
integrals)
  RUN=$2; F=$3
  T=$(ls "$RUN" | grep -E '^[0-9.e+-]+$' | sort -g | tail -1)
  S=$(mktemp -d)
  zcat "$HERE/cases/guided2/polyMesh/points.gz" | awk 'NR>19 && /^\(/{gsub(/[()]/,""); if($1==0&&$2==0)print $3}' > "$S/z.txt"
  for g in 0 1; do
    awk '/^\(/{f=1;next} /^\)/{f=0} f{print $1}' "$RUN/$T/neutroRegion/$F$g" > "$S/f$g.txt"
  done
  # zone of cell c: 0..149 and 301..450 reflector, 150..199 coreLow, 200..249 follower, 250..300 controlRod
  paste "$S/f0.txt" "$S/f1.txt" | awk -v zf="$S/z.txt" 'BEGIN{while((getline z < zf)>0) Z[n++]=z}
    {c=NR-1; v=0.01*(Z[c+1]-Z[c]); k=(c<150||c>300)?0:(c<200?1:(c<250?2:3)); a[k]+=v*$1; b[k]+=v*$2; ta+=v*$1; tb+=v*$2}
    END{for(k=0;k<4;k++) printf "%.10g %.10g\n", a[k]/ta, b[k]/tb}'
  rm -rf "$S"
  ;;
expansion)
  # nuclearData.x0 (README round 47, item 2c): the i0 zones plus xsVariables axExp / radExp (lin) and four states whose zone data are
  # the i0 values times FIXED SYNTHETIC factors (inputs of both codes, not physics): MD MN MR MS scale D, nuSigmaEff and sigmaPow,
  # sigmaRemoval, the scattering matrix.
  st() { # name axExp radExp MD MN MR MS
    echo "    $1"; echo '    {'; echo "        axExp $2;"; echo "        radExp $3;"; echo '        zones'; echo '        ('
    export MD=$4 MN=$5 MR=$6 MS=$7
    zone reflector 2.0 0.3 0.0 0.01 0 0 0.04 1 1 true
    zone coreLow 1.5 0.4 0.01 0.080 0 0.135 0.02 1 1 true
    zone follower 1.5 0.4 0.01 0.085 0 0.135 0.02 1 1 true
    zone controlRod 1.5 0.4 0.01 0.130 0 0.135 0.02 1 1 true
    unset MD MN MR MS
    echo '        );'; echo '    }'
  }
  {
  echo 'FoamFile { version 2.0; format ascii; class dictionary; object nuclearData; }'
  echo '// gh:#1898 round 47: i0 zones with axExp / radExp feedback states (see scripts/make_r47_data.sh expansion)'
  echo 'fastNeutrons    true;'; echo 'adjustDiscFactors false;'; echo 'useGivenDiscFactors false;'; echo 'energyGroups    2;'; echo 'precGroups      6;'
  echo 'xsVariables'; echo '{'; echo '    axExp   lin;'; echo '    radExp  lin;'; echo '}'
  echo 'states'; echo '('
  st reference 0 0 1 1 1 1
  st sa 0.02 0 1.03 0.98 1.02 1.01
  st sr 0 0.02 0.97 1.02 1.01 0.99
  st sb 0.02 0.02 1.01 1.01 1.04 1.02
  echo ');'
  } > "$OUT/nuclearData.x0"
  ;;
esac
