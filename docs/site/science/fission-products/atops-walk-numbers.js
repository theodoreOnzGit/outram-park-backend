// The numbers the three ATOPS concept walks quote, LOOKED UP in recorded
// data, never computed: atops-data.js (the lesson_triso_atops generator,
// boon-lay's port on MHTGR Case A) and atops-walk-recorded.js (the TRISO-ATOPS
// demo's own headless output). The only arithmetic is unit conversion
// (seconds to years or days) and formatting. `numbers(A, R)` returns
// { key: text }; the walk pages bind them by data-q="key", and
// check-concepts.js fails if a key a page uses does not resolve.
(function () {
  "use strict";
  function sci(x) {
    if (x === 0) return "0";
    var s = x.toExponential(2).split("e"), m = s[0], e = Number(s[1]);
    return e === 0 ? m : m + " × 10" + sup(e);
  }
  function sup(e) {
    var map = { "-": "⁻", "0": "⁰", "1": "¹", "2": "²", "3": "³", "4": "⁴", "5": "⁵", "6": "⁶", "7": "⁷", "8": "⁸", "9": "⁹" };
    return String(e).split("").map(function (c) { return map[c]; }).join("");
  }
  function pct(x) { return (100 * x).toPrecision(3) + " %"; }
  function at(xs, ys, x) { var i = xs.indexOf(x); return i < 0 ? NaN : ys[i]; }

  // The demo's CSV as rows keyed "<nuclide>@<hour>" (hour 0 = end of the
  // 40-year normal operation; the last hour = end of the hold).
  function table(csv) {
    var lines = csv.trim().split("\n"), head = lines[0].split(","), out = {}, last = 0;
    lines.slice(1).forEach(function (l) {
      var c = l.split(","), row = {};
      head.forEach(function (h, i) { row[h] = i < 2 ? c[i] : Number(c[i]); });
      out[row.nuclide + "@" + row.hour] = row;
      last = Math.max(last, row.hour);
    });
    return { rows: out, last: last };
  }

  function numbers(A, R) {
    var N = {};
    var nuc = function (n) { return A.nuclides.filter(function (x) { return x.name === n; })[0]; };
    var node = function (n) { return A.nodes.filter(function (x) { return x.name === n; })[0]; };
    var rb = function (n) { return A.rb.filter(function (x) { return x.name === n; })[0]; };
    var curve = function (z) { return A.dCurves.filter(function (d) { return d.z === z; })[0]; };
    var YEAR = 365.25 * 86400, DAY = 86400;
    var cs = nuc("Cs-137"), ag = nuc("Ag-110m"), csN = node("Cs-137"), agN = node("Ag-110m");
    // Pools of a 1 Ci node at 900 °C, MHTGR Case A, no clean-up (atoms; R and S in atoms/s).
    var F = A.poolFields;
    ["Cs-137", "Kr-88", "I-131", "Ag-110m", "Sr-90"].forEach(function (n) {
      var b = node(n).base, k = n.replace(/-/g, "");
      F.forEach(function (f, i) { N["node." + k + "." + f] = sci(b[i]); });
      var h = node(n).hps;
      N["nodeHps." + k + ".cleanUp"] = sci(h[5]);
      N["nodeHps." + k + ".circulating"] = sci(h[3]);
    });
    N["cs.t12y"] = (cs.t12 / YEAR).toFixed(1) + " years";
    N["cs.group"] = cs.group;
    N["cs.z"] = String(cs.z);
    N["cs.dKernel900"] = sci(at(A.dGrid, curve(55).kernel, 900)) + " m²/s";
    N["cs.dGraphite900"] = sci(at(A.dGrid, curve(55).graphite, 900)) + " m²/s";
    N["cs.rb900"] = sci(at(A.rbT, rb("Cs-137").rb, 900));
    N["cs.rb1200"] = sci(at(A.rbT, rb("Cs-137").rb, 1200));
    N["cs.af900"] = sci(at(A.rbT, A.attenuation.Cs, 900));
    N["cs.af1200"] = sci(at(A.rbT, A.attenuation.Cs, 1200));
    N["cs.rVs600"] = sci(at(A.nodeT, csN.releaseVsT, 600));
    N["cs.rVs1200"] = sci(at(A.nodeT, csN.releaseVsT, 1200));
    N["caseA.aBooth"] = (A.caseA.aBooth * 1e6).toFixed(1) + " µm";
    N["caseA.fHM"] = sci(A.caseA.fHM);
    N["caseA.kPlate"] = sci(A.caseA.kPlate) + " /s";
    N["caseA.kClean"] = sci(A.caseA.kClean) + " /s";
    N["caseA.runYears"] = (A.caseA.runTime / YEAR).toFixed(0) + " years";
    N["booth.floor"] = sci(A.booth.long[0]);

    // Silver.
    N["ag.t12d"] = (ag.t12 / DAY).toFixed(0) + " days";
    N["ag.group"] = ag.group;
    [700, 750, 800, 850, 900, 1000, 1100].forEach(function (t) {
      N["ag.bt" + t] = sci(at(A.rbT, A.breakthrough, t));
      N["ag.rb" + t] = sci(at(A.rbT, rb("Ag-110m").rb, t));
    });
    [700, 750, 800, 900, 1000].forEach(function (t) { N["ag.r" + t] = sci(at(A.nodeT, agN.releaseVsT, t)); });
    N["ag.dSic900"] = sci(at(A.dGrid, A.dSicAg, 900)) + " m²/s";
    N["ag.dSic1600"] = sci(at(A.dGrid, A.dSicAg, 1600)) + " m²/s";

    // The accident heat-ups (900 °C to a peak, held, cooled; 5 h samples).
    A.accidents.forEach(function (acc) {
      var p = acc.peak, end = acc.tC.lastIndexOf(p);
      N["acc" + p + ".holdEndH"] = acc.hours[end] + " h";
      N["acc" + p + ".hours"] = acc.hours[acc.hours.length - 1] + " h";
      acc.nuclides.forEach(function (n) {
        var k = n.name.replace(/-/g, "");
        N["acc" + p + "." + k + ".rfKernel"] = pct(n.rfKernel[end]);
        N["acc" + p + "." + k + ".rfGraphite"] = pct(n.rfGraphite[end]);
        N["acc" + p + "." + k + ".perInventory"] = sci(n.releasedPerInventory[end]);
        N["acc" + p + ".vent"] = pct(n.vent[end]);
      });
      // The first cooling sample: venting has stopped (null).
      N["acc" + p + ".ventAfter"] = acc.nuclides[0].vent[end + 1] === null ? "none" : pct(acc.nuclides[0].vent[end + 1]);
    });

    // The demo's HTR-10 numbers (Bq, atoms/s).
    var T = table(R.csv), last = T.last;
    N["demo.holdEnd"] = last + " h";
    ["Cs-137", "I-131", "Kr-88", "Xe-133", "Ag-110m", "Sr-90", "Cs-134"].forEach(function (n) {
      var k = n.replace(/-/g, "");
      [[0, "0"], [last, "end"]].forEach(function (hh) {
        var r = T.rows[n + "@" + hh[0]];
        if (!r) return;
        ["source_atoms_per_s", "circulating_bq", "plated_bq", "hps_bq", "graphite_bq", "release_atoms_per_s", "leaked_atoms"].forEach(function (f) {
          N["demo." + k + "." + hh[1] + "." + f] = sci(r[f]);
        });
      });
    });
    R.sourceTerm.trim().split("\n").slice(1).forEach(function (l) {
      var c = l.split(","), k = c[0].replace(/-/g, "");
      N["st." + k + ".released"] = sci(Number(c[1])) + " Bq";
      N["st." + k + ".core"] = sci(Number(c[2])) + " Bq";
    });
    return N;
  }
  window.ATOPS_WALK_NUMBERS = { numbers: numbers };
})();
