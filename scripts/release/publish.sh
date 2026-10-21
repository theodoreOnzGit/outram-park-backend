#!/usr/bin/env bash
#
# Lockstep release, step 3 of 3: publish every member crate to crates.io
# (gh:#971, #972). Run preflight.sh first.
#
#   scripts/release/publish.sh
#
# Resumable. Each round asks crates.io which crates are already live at their
# new version, excludes them, and runs `cargo publish --workspace` on the
# rest; Cargo orders the crates itself and waits for each to reach the index
# before publishing its dependents. Re-running after a crash or a rate limit
# picks up where it stopped.
#
# Rate limits (docs/workspace-maintenance.md): new crate NAMES get a burst of
# about 5, then about one per 10 minutes; crates.io answers 429 with "Please
# try again after <RFC-2822 date>", and this script sleeps until then. A 503
# from the WAF waits 30 minutes. Anything else stops the script.
#
# Verification is skipped here (--no-verify) because preflight.sh's dry run
# already built every package from its tarball at this same commit.
#
# Replaces scripts/cargo-publish.sh (removed 2026-10-20), whose hand-kept list
# covered 16 of the 48 crates.
#
# The crates.io token must carry the publish-new and publish-update scopes. It
# stays in ~/.cargo/credentials.toml; this script never reads or prints it.

set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
. "$here/common.sh"
cd "$(repo_root)"

log="target/release-logs/publish-$(date +%Y%m%d-%H%M%S).log"
mkdir -p "$(dirname "$log")"
echo "Logging to $log"

for round in $(seq 1 100); do
    excludes=()
    pending=0
    while IFS=$'\t' read -r name ver dir; do
        if version_is_live "$name" "$ver"; then
            excludes+=(--exclude "$name")
        else
            pending=$((pending + 1))
        fi
    done < <(members)

    if [ "$pending" = 0 ]; then
        echo "All crates are live on crates.io." | tee -a "$log"
        exit 0
    fi
    echo "Round $round: $pending crate(s) to publish." | tee -a "$log"

    round_out=$(mktemp)
    set +e
    cargo publish --workspace --no-verify "${excludes[@]}" 2>&1 | tee "$round_out"
    status=${PIPESTATUS[0]}
    set -e
    cat "$round_out" >>"$log"
    if [ "$status" = 0 ]; then rm -f "$round_out"; continue; fi

    # Only this round's output is searched, so an old 429 is never re-read.
    retry_after=$(grep -oE 'try again after [^.]*GMT' "$round_out" | tail -1 | sed 's/try again after //' || true)
    if grep -q '429' "$round_out" && [ -n "$retry_after" ]; then
        wait_s=$(( $(date -d "$retry_after" +%s) - $(date +%s) + 15 ))
        [ "$wait_s" -gt 0 ] || wait_s=15
        echo "Rate limited until $retry_after; sleeping ${wait_s}s." | tee -a "$log"
        rm -f "$round_out"
        sleep "$wait_s"
    elif grep -q '503' "$round_out"; then
        echo "503 from crates.io; sleeping 30 minutes." | tee -a "$log"
        rm -f "$round_out"
        sleep 1800
    else
        rm -f "$round_out"
        die "cargo publish failed for a reason other than a rate limit; see $log"
    fi
done
die "gave up after 100 rounds; see $log"
