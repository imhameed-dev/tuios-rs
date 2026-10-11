#!/usr/bin/env bash
# Publish CI logs as GitHub annotations so results can be read through the API
# (raw job logs are not downloadable from every environment).
#
# GitHub cuts an annotation at about 4 KB and shows at most 10 notices and 10
# errors per step, so: failures go out first as errors, passing test names are
# compressed, and long logs are split into numbered chunks.
#
# Usage: ci-report.sh <logdir>      (needs only bash, sed, awk, grep)
set -u
dir=$1
notice_budget=${CI_REPORT_NOTICES:-9}
error_budget=${CI_REPORT_ERRORS:-9}

clean() { sed 's/\x1b\[[0-9;?]*[A-Za-z]//g' "$1" | tr -d '\r'; }

# chunks <level> <title> <budget-var>: read stdin, print annotation commands.
emit() {
  local level=$1 title=$2 body
  body=$(cat)
  [ -n "$body" ] || body="(no matching lines)"
  local out line
  out=$(printf '%s\n' "$body" | awk -v max=3300 -v title="$title" -v level="$level" '
    function flush() { if (buf != "") { n++; chunk[n] = buf; buf = "" } }
    { l = substr($0, 1, 600)
      if (length(buf) + length(l) + 1 > max) flush()
      buf = buf l "\n" }
    END {
      flush()
      for (i = 1; i <= n; i++) {
        c = chunk[i]; gsub(/%/, "%25", c); gsub(/\n/, "%0A", c)
        t = title; if (n > 1) t = t " (" i "/" n ")"
        printf "::%s title=%s::%s\n", level, t, c
      }
    }')
  while IFS= read -r line; do
    [ -n "$line" ] || continue
    if [ "$level" = error ]; then
      [ "$error_budget" -gt 0 ] || return 0
      error_budget=$((error_budget - 1))
    else
      [ "$notice_budget" -gt 0 ] || return 0
      notice_budget=$((notice_budget - 1))
    fi
    printf '%s\n' "$line"
  done <<<"$out"
}

# Passing tests as "[binary] name, name, ..." plus result lines; failures marked.
compact_tests() {
  awk '
    /^ *Running / { if (names != "") print names; names = ""; sub(/^ *Running /, "[", $0); sub(/ \(.*/, "]", $0); print $0; next }
    /^test result/ { if (names != "") print names; names = ""; print $0; next }
    /^test .* \.\.\. ok$/ { n = $2; sub(/^[a-z_]+::tests::/, "", n); names = (names == "" ? n : names ", " n); next }
    /^test .* \.\.\. (FAILED|ignored)/ { n = $2 " " $4; names = (names == "" ? n : names ", " n); next }
    /SKIPPED/ { print }
    END { if (names != "") print names }'
}

failure_details() {
  grep -A40 -E '^---- .* stdout ----' | head -150
  grep -E 'panicked at|^failures:|^error(\[|:)|^warning: unused|could not compile' | head -40
}

# Failures first, so they are never the part that gets cut.
for f in "$dir"/*.log; do
  [ -f "$f" ] || continue
  name=$(basename "$f" .log)
  case "$name" in
    *test*|*e2e*|*compat*|*perf*|*clippy*|*build*|*fmt*)
      clean "$f" | failure_details | emit error "ci-$name-FAILURE"
      ;;
  esac
done

for f in "$dir"/*.log; do
  [ -f "$f" ] || continue
  name=$(basename "$f" .log)
  case "$name" in
    *test*|*e2e*|*compat*) clean "$f" | compact_tests | emit notice "ci-$name" ;;
    *perf*) clean "$f" | grep -E '^PERF |test result|panicked' | emit notice "ci-$name" ;;
    *deps*|*size*|*env*|*version*|*code*) clean "$f" | head -n 400 | emit notice "ci-$name" ;;
    *) clean "$f" | tail -n 25 | emit notice "ci-$name" ;;
  esac
done
