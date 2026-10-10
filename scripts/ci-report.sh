#!/usr/bin/env bash
# Publish the tail of each CI log as a GitHub annotation so results can be
# read through the API (raw job logs are not always downloadable).
# Usage: ci-report.sh <logdir>
set -u
for f in "$1"/*.log; do
  [ -f "$f" ] || continue
  name=$(basename "$f" .log)
  if [ "$name" = "test" ]; then
    # Test names and results are the evidence; keep every line of those.
    body=$(grep -E '^test |^test result|Running |panicked|FAILED|failures:' "$f" | head -150)
  else
    body=$(tail -n 25 "$f")
  fi
  body=${body//'%'/'%25'}
  body=${body//$'\r'/}
  body=${body//$'\n'/'%0A'}
  echo "::notice title=ci-$name::$body"
done
