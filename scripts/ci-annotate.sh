#!/usr/bin/env bash
# Turn compiler errors and failed tests in a log into GitHub Actions error annotations, so the
# failure is visible on the run summary page (and through the public check-runs API).
# GitHub shows at most 10 error annotations per step, so related lines are grouped.
log="${1:?usage: ci-annotate.sh <logfile>}"
[ -f "$log" ] || exit 0  # the step that writes it did not run

emit() { # title, body
  local body="${2//'%'/'%25'}"
  body="${body//$'\r'/}"
  body="${body//$'\n'/'%0A'}"
  echo "::error title=$1::${body:0:6000}"
}

# Rust compiler errors with 20 lines of context each.
grep -nE '^error(\[E[0-9]+\])?:' "$log" | head -6 | while IFS=: read -r ln _; do
  emit "rustc" "$(sed -n "${ln},$((ln + 20))p" "$log")"
done

# Failed tests: each "---- name stdout ----" section.
grep -nE '^---- .* stdout ----$' "$log" | head -4 | while IFS=: read -r ln _; do
  emit "test failure" "$(sed -n "${ln},$((ln + 25))p" "$log")"
done

# Summary lines (test result: FAILED, failures list), or the log tail if nothing matched.
summary="$(grep -E '^test result: FAILED|^    [a-z_:0-9]+$|panicked at|^failures:|Error: |error: could not compile' "$log" | head -40)"
if [ -n "$summary" ]; then
  emit "summary" "$summary"
else
  emit "log tail" "$(tail -n 40 "$log")"
fi
exit 0
