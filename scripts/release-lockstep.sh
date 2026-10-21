#!/usr/bin/env bash
#
# Lockstep crates.io release with tests ahead of the publish queue
# (gh:#971; maintainer rule, 2026-10-21, recorded in AGENTS.md and
# docs/workspace-maintenance.md "Publishing to crates.io").
#
#   scripts/release-lockstep.sh [--hold crate[,crate...]] [--dry-run]
#
# Env: JOBS          parallel test workers            (default 1)
#      CARGO_BUILD_JOBS  rustc jobs per cargo call    (default 2)
#      RUST_TEST_THREADS test threads per binary      (default 2)
#      VERIFY=0      publish with --no-verify          (default: verify)
#
# What it does:
#   1. Reads the workspace from `cargo metadata`; internal edges are normal,
#      build AND dev dependencies, because `cargo publish` resolves all of them
#      against crates.io. Topologically sorts them (tsort).
#   2. Holds every crate named with --hold and everything that depends on one,
#      transitively over the same edges. Held crates are neither tested nor
#      published.
#   3. A test pool (JOBS workers) runs `cargo test --release -p <c> --lib
#      --tests --no-fail-fast` (default features, so `long-tests` is on) in
#      topological order, low-level crates first.
#   4. Concurrently, ONE serial publisher publishes any crate whose tests are
#      green and whose internal dependencies are all live on crates.io.
#      A red crate is not published, and neither is anything that depends on it.
#
# "Green" includes a crate whose only failures match KNOWN_ENV_FAILURES (the
# environment failures the maintainer listed on 2026-10-21); they are recorded
# in the summary as `known`.
#
# Rate limits (docs/workspace-maintenance.md): new crate NAMES burst 5, then
# one per 10 minutes; new versions are paced at one a minute after a burst of
# 10. A 429 sleeps until crates.io's "try again after" time; a 503 sleeps 30
# minutes then retries; a 403 (expired token) stops the whole run.
#
# Output: release/lockstep-<timestamp>/{summary.tsv,run.log,logs/*.log}.
# The crates.io token stays in ~/.cargo/credentials.toml; never read here.
# Shell, not Python (root CLAUDE.md). Needs jq or jaq.

set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
. "$here/release/common.sh"
cd "$(repo_root)" || exit 1

KNOWN_ENV_FAILURES='citeproc|wgsl_gpu|gpu_langevin|43452|43445'
JOBS=${JOBS:-1}
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-2}
export RUST_TEST_THREADS=${RUST_TEST_THREADS:-2}
VERIFY=${VERIFY:-1}

hold=""; dry=0
while [ $# -gt 0 ]; do
    case $1 in
        --hold) hold=${2//,/ }; shift 2 ;;
        --dry-run) dry=1; shift ;;
        *) die "unknown argument $1" ;;
    esac
done

run="release/lockstep-$(date +%Y%m%d-%H%M%S)"
st="$run/status"; logs="$run/logs"
mkdir -p "$st" "$logs"
log() { echo "$(date -Is) $*" | tee -a "$run/run.log"; }
summary="$run/summary.tsv"
printf 'crate\tversion\ttest\tknown_failures\tpublish\ttimestamp\n' >"$summary"

# ── Graph ────────────────────────────────────────────────────────────────────
meta=$(cargo metadata --no-deps --format-version 1) || die "cargo metadata failed"
echo "$meta" | jq -r '.packages[] | [.name, .version] | @tsv' >"$run/versions.tsv"
echo "$meta" | jq -r '[.packages[].name] as $m | .packages[] | .name as $p
    | .dependencies[] | select(.name as $n | $m | index($n)) | select(.name != $p)
    | [$p, .name, (.kind // "normal")] | @tsv' | sort -u >"$run/edges.tsv"

version_of() { awk -F'\t' -v c="$1" '$1==c{print $2}' "$run/versions.tsv"; }
deps_of() { awk -F'\t' -v c="$1" '$1==c{print $2}' "$run/edges.tsv" | sort -u; }

order=$( { awk -F'\t' '{print $2, $1}' "$run/edges.tsv"; awk -F'\t' '{print $1, $1}' "$run/versions.tsv"; } | tsort) \
    || die "dependency cycle (normal+dev edges); see $run/edges.tsv"
echo "$order" >"$run/order.txt"

# Held closure: anything depending (any kind) on a held crate.
held=" $hold "
while :; do
    grew=0
    while IFS=$'\t' read -r c d k; do
        case $held in *" $d "*) case $held in *" $c "*) ;; *) held+="$c "; grew=1 ;; esac ;; esac
    done <"$run/edges.tsv"
    [ $grew = 0 ] && break
done
log "held: $(echo $held | tr ' ' '\n' | sort | tr '\n' ' ')"

todo=()
for c in $order; do
    case $held in *" $c "*) printf '%s\t%s\theld\t\theld\t%s\n' "$c" "$(version_of "$c")" "$(date -Is)" >>"$summary"; continue ;; esac
    todo+=("$c")
done
log "release order (${#todo[@]}): ${todo[*]}"

# Already live at their local version → nothing to do. New names noted for pacing.
newname=" "
for c in "${todo[@]}"; do
    v=$(version_of "$c")
    case $(api_status "$c") in
        404) newname+="$c " ;;
        200) version_is_live "$c" "$v" && echo live >"$st/$c.pub" ;;
        *) die "crates.io lookup of $c failed" ;;
    esac
done
[ $dry = 1 ] && { log "dry run: stopping before tests"; exit 0; }

# ── Test pool ────────────────────────────────────────────────────────────────
classify() {  # $1 crate, $2 exit code; writes pass|known|fail and known list
    local c=$1 rc=$2 f="$logs/$1.test.log" names n bad=0 known=""
    if [ "$rc" = 0 ]; then echo pass >"$st/$c.test"; return; fi
    names=$(sed -nE 's/^test (.*) \.\.\. FAILED$/\1/p' "$f" | sort -u)
    if [ -z "$names" ] || grep -qE '^error(\[E[0-9]+\])?: (could not compile|aborting)' "$f"; then
        echo fail >"$st/$c.test"; return
    fi
    for n in $names; do
        block=$(awk -v h="---- $n stdout ----" '$0==h{p=1;next} /^---- .* stdout ----$/{p=0} /^failures:$/{p=0} p' "$f")
        if printf '%s\n%s\n' "$n" "$block" | grep -qE "$KNOWN_ENV_FAILURES"; then known+="$n "; else bad=1; fi
    done
    # Every failed binary must be explained by a failed test (no crashes).
    if [ $bad = 0 ] && ! grep -qE 'process didn.t exit successfully: .*(signal|SIG)' "$f"; then
        echo "$known" >"$st/$c.known"; echo known >"$st/$c.test"
    else
        echo fail >"$st/$c.test"
    fi
}
test_one() {
    local c=$1 rc
    log "test start $c"
    cargo test --release -p "$c" --lib --tests --no-fail-fast >"$logs/$c.test.log" 2>&1
    rc=$?
    classify "$c" "$rc"
    log "test end   $c: $(cat "$st/$c.test") (exit $rc)"
}
test_pool() {
    local c
    for c in "${todo[@]}"; do
        [ -f "$st/$c.pub" ] && { echo skip >"$st/$c.test"; continue; }
        [ -f "$run/STOP" ] && break
        while [ "$(jobs -rp | wc -l)" -ge "$JOBS" ]; do wait -n; done
        test_one "$c" &
    done
    wait
}
test_pool &
pool_pid=$!

# ── Serial publisher ─────────────────────────────────────────────────────────
n_new=0; last_new=0; n_upd=0; last_upd=0
pace() {  # $1 = crate
    local now gap
    now=$(date +%s)
    case $newname in
        *" $1 "*) [ $n_new -ge 5 ] && { gap=$((600 - (now - last_new))); [ $gap -gt 0 ] && { log "pace: new-name limit, sleeping ${gap}s"; sleep $gap; }; } ;;
        *) [ $n_upd -ge 10 ] && { gap=$((60 - (now - last_upd))); [ $gap -gt 0 ] && sleep $gap; } ;;
    esac
}
publish_one() {  # returns 0 published, 1 failed, 2 stop
    local c=$1 out="$logs/$1.publish.log" flags=() ra w
    [ "$VERIFY" = 0 ] && flags+=(--no-verify)
    # Tests may rewrite tracked files in their own crate (tampines-steam-tables'
    # generated V&V timestamps); the committed state is what gets published.
    if [ -n "$(git status --porcelain --untracked-files=no -- "crates/$c")" ]; then
        log "restoring test-modified tracked files in crates/$c"; git checkout -- "crates/$c"
    fi
    while :; do
        pace "$c"
        cargo publish -p "$c" "${flags[@]}" >>"$out" 2>&1 && {
            case $newname in *" $c "*) n_new=$((n_new+1)); last_new=$(date +%s) ;; *) n_upd=$((n_upd+1)); last_upd=$(date +%s) ;; esac
            return 0; }
        if grep -q 'already exists' "$out"; then return 0; fi
        if grep -qE '\b403\b' "$out"; then return 2; fi
        ra=$(grep -oE 'try again after [^.]*GMT' "$out" | tail -1 | sed 's/try again after //')
        if grep -qE '\b429\b' "$out" && [ -n "$ra" ]; then
            w=$(( $(date -d "$ra" +%s) - $(date +%s) + 15 )); [ $w -gt 0 ] || w=15
            log "429 on $c; sleeping ${w}s"; echo "--- retry after 429 ---" >>"$out"; sleep $w; continue
        fi
        if grep -qE '\b503\b' "$out"; then
            log "503 on $c; sleeping 30 minutes"; echo "--- retry after 503 ---" >>"$out"; sleep 1800; continue
        fi
        return 1
    done
}
record() {  # crate publish-state
    local c=$1 k=""; [ -f "$st/$c.known" ] && k=$(cat "$st/$c.known")
    echo "$2" >"$st/$c.pub"
    printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$c" "$(version_of "$c")" "$(cat "$st/$c.test" 2>/dev/null)" "$k" "$2" "$(date -Is)" >>"$summary"
    log "publish $c $(version_of "$c"): $2"
}

for c in "${todo[@]}"; do [ "$(cat "$st/$c.pub" 2>/dev/null)" = live ] && record "$c" live; done
while :; do
    pending=0; progress=0
    for c in "${todo[@]}"; do
        [ -f "$st/$c.pub" ] && continue
        pending=1
        [ -f "$st/$c.test" ] || continue
        t=$(cat "$st/$c.test")
        if [ "$t" = fail ]; then record "$c" "not-published:tests-red"; progress=1; continue; fi
        ready=1; blocked=""
        for d in $(deps_of "$c"); do
            case $(cat "$st/$d.pub" 2>/dev/null) in
                live|published) ;;
                "") ready=0 ;;
                *) blocked+="$d " ;;
            esac
        done
        if [ -n "$blocked" ]; then record "$c" "blocked-by:${blocked% }"; progress=1; continue; fi
        [ $ready = 1 ] || continue
        publish_one "$c"; rc=$?
        case $rc in
            0) record "$c" published ;;
            1) record "$c" publish-failed ;;
            2) record "$c" "stopped:403"; touch "$run/STOP"; log "403: token expired; stopping"
               kill "$pool_pid" 2>/dev/null; pkill -P "$pool_pid" 2>/dev/null; exit 3 ;;
        esac
        progress=1
        break   # rescan from the bottom of the order
    done
    [ $pending = 0 ] && break
    [ $progress = 0 ] && sleep 30
done
wait "$pool_pid"
log "done; summary in $summary"
column -t -s$'\t' "$summary"
