#!/usr/bin/env bash
#
# zotero-reference.sh — regenerate the upstream-Zotero reference outputs that
# kovan-literature's translator port is tested against (GitHub #752, #749,
# epic #747).
#
# Needs a RUNNING Zotero translation-server (default http://127.0.0.1:1969;
# set ZOTERO_SERVER to override) started with
#   translatorsDirectory = <vendor>/translators
# (the translators commit the port follows), and Node >= 20. It does not start
# a server and does not install anything: if the server is not answering it
# stops.
#
# Writes crates/kovan-literature/tests/data/zotero/reference/ (replacing it).
# Review the diff before committing: a changed reference means upstream, or
# the fixtures, changed.
#
# What is recorded, and the one normalisation applied (random item keys from
# /import), are described at the top of scripts/zotero-reference.mjs.

set -euo pipefail

server="${ZOTERO_SERVER:-http://127.0.0.1:1969}"
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if ! curl -s -m 10 -o /dev/null -w '%{http_code}' -X POST \
	-H 'Content-Type: text/plain' --data-binary $'TY  - JOUR\nER  - \n' \
	"$server/import" | grep -q '^200$'; then
	echo "zotero-reference: no translation-server answering at $server" >&2
	exit 1
fi

node "$here/zotero-reference.mjs"
echo "zotero-reference: wrote crates/kovan-literature/tests/data/zotero/reference/"
