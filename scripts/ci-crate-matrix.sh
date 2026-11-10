#!/usr/bin/env bash
#
# ci-crate-matrix.sh -- the per-crate job matrix for the long/full test CI
# (GitHub #1337). Read by .github/workflows/per-crate-tests.yml, and locally:
#
#   scripts/ci-crate-matrix.sh                       # every crate, full scope
#   scripts/ci-crate-matrix.sh --package nee_soon    # one crate
#   scripts/ci-crate-matrix.sh --scope fast          # long tests off
#   scripts/ci-crate-matrix.sh --summary             # human-readable list
#
# WHY. GitHub kills a job at 6 hours, and the whole-workspace long tier in
# one job no longer fits in that (full-tests run 42136324698 was cancelled at
# 6 h, still inside outram-mc-libs). One job per crate gives each crate its
# own 6 hours; a crate listed in ci/split-by-target.txt gets one job per test
# target instead.
#
# THE CRATE LIST IS NOT WRITTEN HERE. It is `cargo metadata`'s
# workspace_members, so a new member is tested the moment it is added. No
# member is excluded: the workflows this replaces ran
# `cargo test --workspace`, which excluded nothing.
#
# OUTPUT: one line of JSON, {"include":[...]}, for `strategy.matrix` via
# `fromJSON`. Each entry:
#   id       artifact-safe unique name (package, or package__shard)
#   package  the cargo package (`-p`)
#   shard    "" for a whole-crate job; else "lib", "test/<name>", "bin/<name>"
#   select   the cargo target selection: "--lib --tests" for a whole crate,
#            else "--lib", "--test <name>", "--bin <name>", ...
#
# A SHARD's targets are those `--lib --tests` selects: the lib (always, when
# there is one) and every other target with `test = true`, minus any whose
# `required-features` the scope does not enable (`--tests` skips those
# silently; naming one with `--test` would be an error instead). So the union
# of a crate's shards is exactly what the unsplit job would have run.
#
# Fails if the split list names a non-member, if --package names one, or if
# the matrix exceeds GitHub's 256-job limit per matrix.
#
# Needs bash, cargo and jq (jq is preinstalled on GitHub's ubuntu runners).
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

package=""
scope="full"
split_file="ci/split-by-target.txt"
metadata=""
summary=0
while [ $# -gt 0 ]; do
  case "$1" in
    --package) package="$2"; shift 2 ;;
    --scope) scope="$2"; shift 2 ;;
    --split-file) split_file="$2"; shift 2 ;;
    --metadata) metadata="$2"; shift 2 ;; # a saved `cargo metadata` (tests)
    --summary) summary=1; shift ;;
    -h|--help) sed -n '2,40p' "$0"; exit 0 ;;
    *) echo "ci-crate-matrix.sh: unknown argument: $1" >&2; exit 2 ;;
  esac
done
case "$scope" in
  full|fast) ;;
  *) echo "ci-crate-matrix.sh: --scope must be full or fast (got: $scope)" >&2; exit 2 ;;
esac

if [ -n "$metadata" ]; then
  meta=$(cat "$metadata")
else
  meta=$(cargo metadata --format-version 1 --no-deps)
fi

# The split list: package names, comments and blank lines dropped.
split_json=$(sed -e 's/#.*//' -e 's/[[:space:]]*$//' -e '/^[[:space:]]*$/d' "$split_file" \
  | jq -R . | jq -s .)

# shellcheck disable=SC2016 # jq program, not shell
matrix=$(jq -c --argjson split "$split_json" --arg only "$package" --arg scope "$scope" '
  # The features a scope enables: full = the default set, closed over the
  # features it names (plain names only: "dep:x" and "x/f" switch on
  # dependencies, never one of this package'\''s own features); fast = none,
  # as `--no-default-features` does.
  def own: map(select(test("[:/]") | not)) | unique;
  def step($pkg): [ .[], (.[] | ($pkg.features[.] // [])[]) ] | own;
  def enabled($pkg):
    if $scope == "fast" then []
    else [ $pkg.features.default // [] | .[] ] | own | until(. == step($pkg); step($pkg))
    end;
  def libkinds: ["lib","rlib","dylib","cdylib","staticlib","proc-macro"];
  def shards($pkg):
    enabled($pkg) as $on
    | [ $pkg.targets[]
        | select(((."required-features" // []) - $on) | length == 0)
        | (.kind[0]) as $k
        | if (libkinds | index($k)) then {shard: "lib", select: "--lib", order: 0}
          elif .test then
            {shard: "\($k)/\(.name)", select: "--\($k) \(.name)", order: 1}
          else empty end ]
    | unique_by(.shard) | sort_by(.order, .shard) | map(del(.order));

  (.workspace_members) as $members
  | [ .packages[] | select(.id as $id | $members | index($id)) ] as $pkgs
  | ($pkgs | map(.name)) as $names
  | ($split - $names) as $unknown
  | if ($unknown | length) > 0 then
      error("ci/split-by-target.txt names non-members: \($unknown | join(", "))")
    elif $only != "" and ($names | index($only) | not) then
      error("--package \($only) is not a workspace member")
    else . end
  | [ $pkgs[]
      | select($only == "" or .name == $only)
      | . as $pkg
      | if ($split | index($pkg.name)) then
          shards($pkg)[]
          | {id: ("\($pkg.name)__\(.shard)" | gsub("[^A-Za-z0-9_.-]"; "_")),
             package: $pkg.name, shard: .shard, select: .select}
        else
          {id: $pkg.name, package: $pkg.name, shard: "", select: "--lib --tests"}
        end ]
  | sort_by(.package, .shard)
  | if length > 256 then error("matrix has \(length) jobs; GitHub allows 256 per matrix") else . end
  | {include: .}
' <<<"$meta")

if [ "$summary" = 1 ]; then
  jq -r '.include[] | "\(.package)\t\(.select)"' <<<"$matrix"
  jq -r '"jobs: \(.include | length) (\([.include[] | select(.shard == "")] | length) whole crates, \([.include[] | select(.shard != "")] | length) shards of \([.include[] | select(.shard != "") | .package] | unique | length) split crates)"' <<<"$matrix" >&2
else
  printf '%s\n' "$matrix"
fi
