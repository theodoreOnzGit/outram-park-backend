#!/usr/bin/env bash
# The site search index (gh:#909, #906 spec §2.1): one JSON array of
# {"k": "science"|"code"|"demo", "t": title, "u": url from the site root,
# "x": extra search terms}, written to <out>/search.json and read by
# docs/site/search.js. Built from what the site already builds, never from a
# second hand-kept list:
#
# - every chapter of a tutorial book (docs/site/tutorials.txt) is "science",
#   every chapter of a deep-dive book (docs/site/deep-dives.txt) is "code";
#   a chapter's title comes from its book's SUMMARY.md and its terms are the
#   chapter's own headings;
# - every link marked `data-kind="..."` on a pathway page (science/, code/,
#   demos/ under docs/site/) is an entry of that kind, with its
#   `data-terms`. Such a link must sit on one line.
#
# The crates themselves are searched from code-map/code_map.json at run time
# (search.js), so they are not repeated here.
#
#   scripts/build-search-index.sh <out-dir>
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
OUT="${1:?usage: build-search-index.sh <out-dir>}"

# JSON string body: backslashes, quotes, tabs; control characters dropped.
esc() { printf '%s' "$1" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g' -e 's/\t/ /g' | tr -d '\000-\037'; }
sep=""
emit() { # kind title url terms
  printf '%s{"k":"%s","t":"%s","u":"%s","x":"%s"}' "$sep" "$1" "$(esc "$2")" "$(esc "$3")" "$(esc "$4")"
  sep=$',\n'
}

# A book's chapters: "<title>\t<file.md>" per SUMMARY.md link.
chapters() {
  sed -nE 's/^[[:space:]]*-?[[:space:]]*\[([^]]+)\]\(\.?\/?([^)]+\.md)\).*/\1\t\2/p' "$1/src/SUMMARY.md"
}
# A chapter's headings, flattened into one line of terms.
headings() {
  [[ -f "$1" ]] || return 0
  grep -E '^#{1,4} ' "$1" | sed -E 's/^#+ +//; s/[`*_]//g' | tr '\n' ' '
}

book_entries() { # list kind url-prefix
  local name dir title file url
  while read -r name dir; do
    [[ -z "$name" || "$name" == \#* ]] && continue
    while IFS=$'\t' read -r title file; do
      url="$2/$name/${file%.md}.html"
      [[ "$file" == README.md ]] && url="$2/$name/index.html"
      emit "$3" "$title" "$url" "$(headings "$dir/src/$file")"
    done < <(chapters "$dir")
  done < "$1"
}

# Links marked data-kind on a pathway page, with the page's directory
# (relative to the site root) to resolve them against.
page_entries() { # page.html dir-from-root
  local line href kind terms title
  while IFS= read -r line; do
    href="$(sed -nE 's/.*href="([^"]+)".*/\1/p' <<< "$line")"
    kind="$(sed -nE 's/.*data-kind="([^"]+)".*/\1/p' <<< "$line")"
    terms="$(sed -nE 's/.*data-terms="([^"]*)".*/\1/p' <<< "$line")"
    title="$(sed -E 's/.*<a [^>]*>(.*)<\/a>.*/\1/; s/<[^>]+>//g; s/&amp;/\&/g; s/&mdash;/—/g; s/&ndash;/–/g; s/&alpha;/α/g; s/&beta;/β/g; s/&sigma;/σ/g; s/&lambda;/λ/g; s/&infin;/∞/g' <<< "$line")"
    href="${href//&amp;/&}"
    case "$href" in
      http*) ;;
      ../*) href="${href#../}" ;;
      *) href="$2$href" ;;
    esac
    emit "$kind" "$title" "$href" "$terms"
  done < <(grep -E '<a [^>]*data-kind="' "$1")
}

{
  printf '[\n'
  book_entries docs/site/tutorials.txt tutorials science
  book_entries docs/site/deep-dives.txt deep-dives code
  page_entries docs/site/science/index.html science/
  page_entries docs/site/code/index.html code/
  page_entries docs/site/demos/index.html demos/
  printf '\n]\n'
} > "$OUT/search.json"
