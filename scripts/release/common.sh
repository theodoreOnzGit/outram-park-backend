# Shared helpers for the lockstep release scripts (gh:#971, #972). Sourced, not
# run. Shell, not Python, per the workspace "No Python for documentation or
# accounting" rule.

# jq, or jaq (`cargo install jaq`) where jq is not installed; the filters used
# here behave the same in both.
if ! command -v jq >/dev/null 2>&1 && command -v jaq >/dev/null 2>&1; then
    jq() { jaq "$@"; }
fi

UA="outram-park-release (https://github.com/theodoreOnzGit/outram-park-backend)"
CRATES_API="https://crates.io/api/v1/crates"
OWNER="theodoreOnzGit"

repo_root() { git rev-parse --show-toplevel; }

# name<TAB>version<TAB>manifest_dir, one line per workspace member.
members() {
    cargo metadata --no-deps --format-version 1 \
        | jq -r '.packages[] | [.name, .version, (.manifest_path | sub("/Cargo.toml$"; ""))] | @tsv'
}

# HTTP status of a crates.io API path. crates.io asks crawlers for at most one
# request per second, so every call sleeps one second afterwards.
api_status() {
    local code
    code=$(curl -s -o /dev/null -w '%{http_code}' -A "$UA" "$CRATES_API/$1")
    sleep 1
    echo "$code"
}

# 0 when <name> <version> is live on crates.io.
version_is_live() { [ "$(api_status "$1/$2")" = 200 ]; }

# Owners' logins of a crate, one per line (empty for a crate not on crates.io).
owners() {
    curl -s -A "$UA" "$CRATES_API/$1/owners" | jq -r '.users[]?.login'
    sleep 1
}

# 0 when version $1 sorts strictly after version $2.
version_gt() { [ "$1" != "$2" ] && [ "$(printf '%s\n%s\n' "$1" "$2" | sort -V | tail -1)" = "$1" ]; }

die() { echo "error: $*" >&2; exit 1; }
