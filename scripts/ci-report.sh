#!/usr/bin/env bash
# Publish each CI log as a GitHub annotation so results can be read through the
# API (raw job logs are not always downloadable). Usage: ci-report.sh <logdir>
set -u
for f in "$1"/*.log; do
  [ -f "$f" ] || continue
  name=$(basename "$f" .log)
  clean=$(mktemp)
  sed 's/\x1b\[[0-9;?]*[A-Za-z]//g' "$f" > "$clean"
  case "$name" in
    *test*|*e2e*|*compat*)
      body=$({ grep -E '^test |^test result|Running |panicked|FAILED|failures:|SKIPPED' "$clean" | head -200
               grep -A25 -E '^---- .* stdout ----' "$clean" | head -200; })
      ;;
    *perf*) body=$(grep -E '^PERF |test result|panicked' "$clean" | head -100) ;;
    *deps*|*size*|*env*|*version*|*code*) body=$(head -n 400 "$clean") ;;
    *) body=$(tail -n 25 "$clean") ;;
  esac
  rm -f "$clean"
  [ -n "$body" ] || body="(no matching lines)"
  body=${body:0:60000}
  body=${body//'%'/'%25'}
  body=${body//$'\r'/}
  body=${body//$'\n'/'%0A'}
  echo "::notice title=ci-$name::$body"
done
