#!/usr/bin/env bash
#
# Post-process the mdBook output in `book/` for search and AI-answer engines.
#
# mdBook renders `theme/head.hbs` with an identical context on every page, so
# anything page-specific has to be injected afterwards. This script:
#
#   1. adds a self-referencing <link rel="canonical"> and <meta og:url> to every
#      page  — without these, either every page canonicalises to the site root
#      (which de-indexes the whole book) or there is no canonical at all;
#   2. marks mdBook's generated `print.html` noindex — it is a concatenation of
#      every chapter and is otherwise the largest duplicate-content page on the
#      site;
#   3. attaches FAQPage structured data to the FAQ, so answer engines can lift
#      the questions directly;
#   4. generates `sitemap.xml` from the pages that actually exist, rather than
#      from a hand-maintained list that drifts out of date;
#   5. copies `robots.txt` and `llms.txt` to the site root.
#
# Usage: scripts/postprocess-docs.sh [book-dir]
set -euo pipefail

BOOK_DIR="${1:-book}"
BASE_URL="https://vaibhavgupta9877.github.io/ruprizzle-orm/"

if [ ! -d "$BOOK_DIR" ]; then
  echo "postprocess-docs: '$BOOK_DIR' does not exist; run 'mdbook build' first" >&2
  exit 1
fi

# Pages that must never be advertised to a crawler: mdBook's 404 stub and the
# print view. Both are excluded from the sitemap; print.html also gets noindex.
# mdBook redirect stubs ([output.html.redirect]) are meta-refresh pages that
# already carry their own canonical. Injecting a second one, or listing them in
# the sitemap, would advertise a URL whose only content is a redirect.
is_redirect() {
  grep -qi 'http-equiv="refresh"' "$1"
}

is_excluded() {
  case "$1" in
    404.html | print.html) return 0 ;;
    *) return 1 ;;
  esac
}

# book/a/b/index.html -> a/b/    |    book/a/b.html -> a/b.html
url_path_for() {
  local rel="$1"
  case "$rel" in
    index.html) printf '' ;;
    */index.html) printf '%s' "${rel%index.html}" ;;
    *) printf '%s' "$rel" ;;
  esac
}

# Splice markup in immediately before the first literal `</head>`, wherever it
# falls on the line — mdBook normally puts it on its own line, but a theme
# change must not silently push our tags outside the document head.
insert_before_head() {
  local file="$1"
  # Passed through the environment, not `awk -v`: a -v assignment runs escape
  # processing over the value, which turns the \" sequences in the JSON-LD
  # payload back into bare quotation marks and produces invalid structured data.
  local RZ_PAYLOAD="$2"
  export RZ_PAYLOAD
  awk '
    BEGIN { payload = ENVIRON["RZ_PAYLOAD"] }
    !done {
      i = index($0, "</head>")
      if (i) {
        print substr($0, 1, i - 1) "\n" payload "\n" substr($0, i)
        done = 1
        next
      }
    }
    { print }
    END { if (!done) exit 3 }
  ' "$file" > "$file.tmp" || {
    echo "postprocess-docs: no </head> found in $file" >&2
    rm -f "$file.tmp"
    return 1
  }
  mv "$file.tmp" "$file"
}

# Idempotent wrapper: re-running the script over an already-processed tree is a
# no-op rather than a second set of canonical tags.
inject_head() {
  local file="$1" payload="$2"
  if grep -q 'data-ruprizzle-seo' "$file"; then
    return 0
  fi
  insert_before_head "$file" "$payload"
}

pages=()
while IFS= read -r file; do
  pages+=("$file")
done < <(find "$BOOK_DIR" -type f -name '*.html' | sort)

echo "postprocess-docs: found ${#pages[@]} HTML pages in $BOOK_DIR"

for file in "${pages[@]}"; do
  rel="${file#"$BOOK_DIR"/}"
  if is_redirect "$file"; then continue; fi
  url="${BASE_URL}$(url_path_for "$rel")"

  payload="<!-- data-ruprizzle-seo -->
<link rel=\"canonical\" href=\"$url\">
<meta property=\"og:url\" content=\"$url\">"

  if [ "$rel" = "print.html" ]; then
    payload="$payload
<meta name=\"robots\" content=\"noindex, follow\">"
  fi

  inject_head "$file" "$payload"
done

# ---------------------------------------------------------------------------
# FAQPage structured data
#
# Built from the FAQ's own <h2> headings so the markup cannot drift away from
# the visible copy — Google requires the two to match.
# ---------------------------------------------------------------------------
faq="$BOOK_DIR/faq.html"
if [ -f "$faq" ] && ! grep -q 'FAQPage' "$faq"; then
  # Pair each `?`-terminated <h2> with the prose that follows it, up to the next
  # heading. Google requires the schema answer to be the answer the reader sees,
  # so the text is lifted from the rendered page rather than written by hand.
  entities=$(
    awk '
      # JSON string escaping, done by concatenation rather than gsub: a gsub
      # replacement treats backslash specially and silently drops it, which
      # produces invalid JSON the moment an answer contains a quotation mark.
      function jsonesc(s,   i, c, out) {
        out = ""
        for (i = 1; i <= length(s); i++) {
          c = substr(s, i, 1)
          if (c == "\\") out = out "\\\\"
          else if (c == "\"") out = out "\\\""
          else out = out c
        }
        return out
      }
      function strip(s) {
        gsub(/<[^>]*>/, "", s)
        gsub(/&lt;/, "<", s); gsub(/&gt;/, ">", s)
        gsub(/&quot;/, "\"", s); gsub(/&#39;/, "'"'"'", s); gsub(/&nbsp;/, " ", s)
        gsub(/&amp;/, "\\&", s)
        gsub(/[ \t]+/, " ", s); gsub(/^ +| +$/, "", s)
        return s
      }
      function flush(   a) {
        if (q == "") return
        a = strip(buf)
        if (a == "") return
        if (length(a) > 320) a = substr(a, 1, 317) "..."
        if (n++) printf ",\n"
        printf "    {\"@type\":\"Question\",\"name\":\"%s\",\"acceptedAnswer\":{\"@type\":\"Answer\",\"text\":\"%s\"}}", jsonesc(q), jsonesc(a)
        q = ""; buf = ""
      }
      /<h[1-6][ >]/ {
        flush()
        line = $0
        sub(/.*<h[1-6][^>]*>/, "", line)
        sub(/<a class="header".*/, "", line)
        sub(/<\/h[1-6]>.*/, "", line)
        cand = strip(line)
        if (cand ~ /\?$/) q = cand
        next
      }
      q != "" { buf = buf " " $0 }
      END { flush(); if (n) printf "\n" }
    ' "$faq"
  )
  if [ -n "$entities" ]; then
    questions=$(printf '%s\n' "$entities" | grep -c '"@type":"Question"')
    insert_before_head "$faq" "<script type=\"application/ld+json\">
{\"@context\":\"https://schema.org\",\"@type\":\"FAQPage\",\"mainEntity\":[
$entities
]}
</script>"
    echo "postprocess-docs: attached FAQPage schema ($questions questions)"
  fi
fi

# ---------------------------------------------------------------------------
# sitemap.xml — generated from the pages that exist, never hand-maintained
# ---------------------------------------------------------------------------
lastmod=$(date -u +%Y-%m-%d)
{
  echo '<?xml version="1.0" encoding="UTF-8"?>'
  echo '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">'
  for file in "${pages[@]}"; do
    rel="${file#"$BOOK_DIR"/}"
    if is_excluded "$rel" || is_redirect "$BOOK_DIR/$rel"; then
      continue
    fi
    echo '  <url>'
    echo "    <loc>${BASE_URL}$(url_path_for "$rel")</loc>"
    echo "    <lastmod>$lastmod</lastmod>"
    echo '  </url>'
  done
  echo '</urlset>'
} > "$BOOK_DIR/sitemap.xml"

count=$(grep -c '<loc>' "$BOOK_DIR/sitemap.xml")
echo "postprocess-docs: wrote sitemap.xml with $count URLs"

# ---------------------------------------------------------------------------
# Root files for crawlers and AI agents
# ---------------------------------------------------------------------------
for f in robots.txt llms.txt; do
  if [ -f "$f" ]; then
    cp "$f" "$BOOK_DIR/$f"
    echo "postprocess-docs: copied $f"
  else
    echo "postprocess-docs: WARNING $f not found at repository root" >&2
  fi
done

echo "postprocess-docs: done"
